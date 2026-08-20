package main

import (
	"flag"
	"fmt"
	"io"
	"log"
	"net/http"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"time"
)

func main() {
	addr := flag.String("addr", "127.0.0.1:8787", "address to bind (localhost only unless changed explicitly)")
	jobs := flag.String("jobs", jobsDirDefault(), "directory of autopilot job plists")
	flag.Parse()

	if !strings.HasPrefix(*addr, "127.0.0.1") && !strings.HasPrefix(*addr, "localhost") {
		log.Fatalf("refusing to bind %s: the dashboard serves an operator's projects and is localhost-only by default", *addr)
	}

	mux := http.NewServeMux()
	mux.HandleFunc("/", overviewHandler(*jobs))
	mux.HandleFunc("/project/", projectHandler(*jobs))
	mux.HandleFunc("/fragment/live", liveHandler(*jobs))
	mux.HandleFunc("/htmx.min.js", func(w http.ResponseWriter, r *http.Request) {
		f, err := uiFS.Open("ui/htmx.min.js")
		if err != nil {
			http.NotFound(w, r)
			return
		}
		defer f.Close()
		w.Header().Set("Content-Type", "text/javascript")
		_, _ = io.Copy(w, f)
	})

	log.Printf("dashboard listening on http://%s (read-only)", *addr)
	if err := http.ListenAndServe(*addr, mux); err != nil {
		log.Fatal(err)
	}
}

func jobsDirDefault() string {
	home, _ := os.UserHomeDir()
	return filepath.Join(home, ".local/share/autopilot/jobs")
}

func buildViews(jobs string) []ProjectView {
	projects := Projects(jobs)
	views := make([]ProjectView, 0, len(projects))
	for _, p := range projects {
		v := ProjectView{
			Label:    p.Label,
			Path:     p.Path,
			Loaded:   p.Loaded,
			Runs:     p.Runs,
			LastExit: p.LastExit,
			Err:      p.Err,
			Cost:     map[string]TierCost{},
		}
		if p.Path != "" {
			if _, err := os.Stat(filepath.Join(p.Path, ".autopilot/STOP")); err == nil {
				v.STOP = true
			}
			evs, err := readJournal(p.Path)
			switch {
			case err == nil:
				v.Orphans = Orphans(evs, time.Now())
				v.Cost = CostByTier(evs)
				v.InFlight = inFlight(evs)
			case os.IsNotExist(err):
				// No task has run yet; an empty view is the honest one.
			default:
				// A parse error is exactly the kind of "wrong in either
				// direction" status this dashboard exists to avoid — showing
				// nothing here would hide the same orphan detection the
				// overview's headline feature depends on.
				v.Err = err.Error()
			}
		}
		views = append(views, v)
	}
	return views
}

// readJournal reads every journal file the runner has written: the current
// journal.jsonl plus any rolled journal-<date>.jsonl files. A roll renames the
// old file away, so a reader that stopped at journal.jsonl went blind the
// moment the journal first grew past its size cap — orphan detection and cost
// figures silently covered only the newest file.
func readJournal(path string) ([]Event, error) {
	dir := filepath.Join(path, ".autopilot")
	if _, err := os.Stat(filepath.Join(dir, "journal.jsonl")); err != nil {
		return nil, err // os.IsNotExist → "no task has run yet" to the caller
	}
	files, err := journalFiles(dir)
	if err != nil {
		return nil, err
	}
	var events []Event
	for _, f := range files {
		fh, err := os.Open(f)
		if err != nil {
			return nil, err
		}
		evs, perr := ParseJournal(fh)
		fh.Close()
		if perr != nil {
			return nil, fmt.Errorf("%s: %w", f, perr)
		}
		events = append(events, evs...)
	}
	return events, nil
}

// journalFiles lists the current journal.jsonl first, then the rolled
// journal-<date>.jsonl files in name order. Consumers only count/map, so the
// order is cosmetic, but the current file first keeps the file that is still
// being appended at the front of any event stream.
func journalFiles(dir string) ([]string, error) {
	entries, err := os.ReadDir(dir)
	if err != nil {
		return nil, err
	}
	var rolled []string
	for _, e := range entries {
		name := e.Name()
		if strings.HasPrefix(name, "journal-") && strings.HasSuffix(name, ".jsonl") {
			rolled = append(rolled, filepath.Join(dir, name))
		}
	}
	sort.Strings(rolled)
	return append([]string{filepath.Join(dir, "journal.jsonl")}, rolled...), nil
}

// inFlight counts started-but-not-ended roles. Keyed the same way Orphans is
// (wake, role, round, lens) — a coarser wake+role key collapses a role's
// second round onto its first, so a still-running round 2 reads as not in
// flight once round 1's role_end has already landed.
func inFlight(evs []Event) int {
	started := map[roleKey]bool{}
	ended := map[roleKey]bool{}
	for _, ev := range evs {
		k := roleKey{ev.Wake, ev.Role, ev.Round, ev.Lens}
		if ev.Event == "role_start" {
			started[k] = true
		}
		if ev.Event == "role_end" {
			ended[k] = true
		}
	}
	n := 0
	for k := range started {
		if !ended[k] {
			n++
		}
	}
	return n
}

func overviewHandler(jobs string) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path != "/" {
			http.NotFound(w, r)
			return
		}
		html, err := renderOverview(buildViews(jobs), time.Now())
		if err != nil {
			http.Error(w, err.Error(), http.StatusInternalServerError)
			return
		}
		w.Header().Set("Content-Type", "text/html; charset=utf-8")
		_, _ = io.WriteString(w, html)
	}
}

func liveHandler(jobs string) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		html, err := renderOverview(buildViews(jobs), time.Now())
		if err != nil {
			http.Error(w, err.Error(), http.StatusInternalServerError)
			return
		}
		_, _ = io.WriteString(w, html)
	}
}

func projectHandler(jobs string) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		label := strings.TrimPrefix(r.URL.Path, "/project/")
		for _, p := range Projects(jobs) {
			if p.Label != label {
				continue
			}
			evs, err := readJournal(p.Path)
			if err != nil {
				http.Error(w, err.Error(), http.StatusInternalServerError)
				return
			}
			html, err := renderTask(evs)
			if err != nil {
				http.Error(w, err.Error(), http.StatusInternalServerError)
				return
			}
			w.Header().Set("Content-Type", "text/html; charset=utf-8")
			_, _ = io.WriteString(w, html)
			return
		}
		http.NotFound(w, r)
	}
}
