package main

import (
	"strings"
	"testing"
	"time"
)

func mustTime(s string) time.Time {
	t, _ := time.Parse(time.RFC3339, s)
	return t
}

func TestAnOrphanedRoleIsFound(t *testing.T) {
	in := `{"ts":"2026-08-16T01:00:00Z","wake":"w-1","event":"role_start","role":"review","round":1,"tier":"deep","lens":"partial-failure"}
{"ts":"2026-08-16T01:00:05Z","wake":"w-1","event":"role_start","role":"test","round":1,"tier":"light"}
{"ts":"2026-08-16T01:02:00Z","wake":"w-1","event":"role_end","role":"test","round":1,"tier":"light","classify":"ok","cost_usd":0.1,"cost_source":"reported","duration_s":115}`
	evs, err := ParseJournal(strings.NewReader(in))
	if err != nil {
		t.Fatal(err)
	}
	o := Orphans(evs, mustTime("2026-08-16T02:00:00Z"))
	if len(o) != 1 {
		t.Fatalf("want 1 orphan, got %d", len(o))
	}
	if o[0].Role != "review" || o[0].Lens != "partial-failure" {
		t.Fatalf("wrong orphan: %+v", o[0])
	}
	if o[0].Age < time.Hour {
		t.Fatalf("age must be measured from role_start, got %v", o[0].Age)
	}
}

func TestATruncatedFinalLineIsNotFatal(t *testing.T) {
	in := `{"ts":"2026-08-16T01:00:00Z","wake":"w-1","event":"wake_start","issue":4,"tier":"light"}
{"ts":"2026-08-16T01:00:05Z","wake":"w-1","event":"role_st`
	evs, err := ParseJournal(strings.NewReader(in))
	if err != nil {
		t.Fatalf("a truncated tail must not fail the parse: %v", err)
	}
	if len(evs) != 1 {
		t.Fatalf("want the 1 complete event, got %d", len(evs))
	}
}

func TestCostsOfDifferentProvenanceAreNeverSummed(t *testing.T) {
	in := `{"event":"role_end","tier":"deep","cost_usd":1.5,"cost_source":"reported"}
{"event":"role_end","tier":"deep","cost_usd":null,"cost_source":"unknown"}`
	evs, _ := ParseJournal(strings.NewReader(in))
	c := CostByTier(evs)["deep"]
	if c.Reported != 1.5 {
		t.Fatalf("reported: want 1.5, got %v", c.Reported)
	}
	if c.UnknownCount != 1 {
		t.Fatalf("unknown must be counted, not zeroed")
	}
}
