package main

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func writePlistFixture(t *testing.T, jobsDir, label, projectPath string) {
	t.Helper()
	plist := "<key>ProgramArguments</key><array><string>--project</string><string>" +
		projectPath + "</string></array>"
	if err := os.WriteFile(filepath.Join(jobsDir, label+".plist"), []byte(plist), 0o644); err != nil {
		t.Fatal(err)
	}
}

func TestInFlightDistinguishesRoundsLikeOrphansDoes(t *testing.T) {
	// round 1 of "test" finished; round 2 of "test" is still running. A
	// wake+role key (ignoring round) would see role_end for "test" and
	// report nothing in flight, hiding the still-running round 2.
	in := `{"wake":"w-1","event":"role_start","role":"test","round":1}
{"wake":"w-1","event":"role_end","role":"test","round":1}
{"wake":"w-1","event":"role_start","role":"test","round":2}`
	evs, err := ParseJournal(strings.NewReader(in))
	if err != nil {
		t.Fatal(err)
	}
	if n := inFlight(evs); n != 1 {
		t.Fatalf("want 1 role in flight (round 2), got %d", n)
	}
}

func TestBuildViewsSurfacesAGenuineParseErrorRatherThanHidingIt(t *testing.T) {
	jobs := t.TempDir()
	proj := filepath.Join(t.TempDir(), "proj")
	if err := os.MkdirAll(filepath.Join(proj, ".autopilot"), 0o755); err != nil {
		t.Fatal(err)
	}
	// Not the last line, and not valid JSON: a genuine parse error, not a
	// truncated tail.
	bad := "{\"event\":\"role_start\"\n{\"event\":\"role_end\"}\n"
	if err := os.WriteFile(filepath.Join(proj, ".autopilot/journal.jsonl"), []byte(bad), 0o644); err != nil {
		t.Fatal(err)
	}
	writePlistFixture(t, jobs, "demo", proj)

	views := buildViews(jobs)
	if len(views) != 1 {
		t.Fatalf("want 1 project view, got %d", len(views))
	}
	if views[0].Err == "" {
		t.Fatal("a broken journal must surface as an error, not render as an empty/healthy project")
	}
}

func TestBuildViewsTreatsAMissingJournalAsEmptyNotAnError(t *testing.T) {
	jobs := t.TempDir()
	proj := filepath.Join(t.TempDir(), "proj")
	if err := os.MkdirAll(filepath.Join(proj, ".autopilot"), 0o755); err != nil {
		t.Fatal(err)
	}
	writePlistFixture(t, jobs, "demo", proj)

	views := buildViews(jobs)
	if len(views) != 1 {
		t.Fatalf("want 1 project view, got %d", len(views))
	}
	if views[0].Err != "" {
		t.Fatalf("a project that has never run yet must not show as errored: %q", views[0].Err)
	}
}

func TestBuildViewsCountsOrphansAcrossRolledJournals(t *testing.T) {
	jobs := t.TempDir()
	proj := filepath.Join(t.TempDir(), "proj")
	if err := os.MkdirAll(filepath.Join(proj, ".autopilot"), 0o755); err != nil {
		t.Fatal(err)
	}
	// The orphan lives in a rolled journal; the current journal is clean.
	cur := "{\"wake\":\"w-2\",\"event\":\"role_start\",\"role\":\"test\",\"round\":0}\n" +
		"{\"wake\":\"w-2\",\"event\":\"role_end\",\"role\":\"test\",\"round\":0}\n"
	rolled := "{\"wake\":\"w-1\",\"event\":\"role_start\",\"role\":\"review\",\"round\":1}\n"
	if err := os.WriteFile(filepath.Join(proj, ".autopilot/journal.jsonl"), []byte(cur), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(proj, ".autopilot/journal-2026-08-19.jsonl"), []byte(rolled), 0o644); err != nil {
		t.Fatal(err)
	}
	writePlistFixture(t, jobs, "demo", proj)

	views := buildViews(jobs)
	if len(views) != 1 {
		t.Fatalf("want 1 project view, got %d", len(views))
	}
	if views[0].Err != "" {
		t.Fatalf("rolled journals must not break the view: %q", views[0].Err)
	}
	if len(views[0].Orphans) != 1 {
		t.Fatalf("want 1 orphan from the rolled journal, got %d", len(views[0].Orphans))
	}
	if views[0].Orphans[0].Role != "review" {
		t.Fatalf("want the review orphan, got %q", views[0].Orphans[0].Role)
	}
}
