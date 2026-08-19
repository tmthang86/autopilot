package main

import (
	"strings"
	"testing"
	"time"
)

func TestOrphansRenderAboveEverythingElse(t *testing.T) {
	views := []ProjectView{{
		Label:   "com.autopilot.x",
		Path:    "/tmp/x",
		Loaded:  true,
		Runs:    3,
		Orphans: []Orphan{{Role: "review", Lens: "plan-conformance", Age: 3 * time.Hour}},
		Cost:    map[string]TierCost{"deep": {Reported: 1.5}},
	}}
	html, err := renderOverview(views, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	if strings.Index(html, "orphaned roles") > strings.Index(html, "cost by tier") {
		t.Fatal("orphans must render before costs")
	}
}

func TestALoadedJobWithZeroRunsIsCalledOut(t *testing.T) {
	views := []ProjectView{{Label: "com.autopilot.x", Path: "/tmp/x", Loaded: true, Runs: 0, LastExit: 78}}
	html, err := renderOverview(views, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(html, "loaded but has never run") {
		t.Fatal("a job that loads and never runs must be named")
	}
}

func TestTheTwoCostFiguresAreShownSeparately(t *testing.T) {
	views := []ProjectView{{Label: "x", Cost: map[string]TierCost{"deep": {Reported: 1.5, UnknownCount: 2}}}}
	html, err := renderOverview(views, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(html, "reported") || !strings.Contains(html, "unknown") {
		t.Fatal("cost provenance must be shown separately")
	}
}

func TestARejectedVerdictShowsItsReasonInFull(t *testing.T) {
	long := strings.Repeat("why not ", 20)
	events := []Event{{Event: "verdict", Role: "review", Verdict: "reject", Reason: long}}
	html, err := renderTask(events)
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(html, long) {
		t.Fatal("a verdict reason must not be truncated")
	}
}
