// Package main reads autopilot run journals and renders them. It never writes.
package main

import (
	"bufio"
	"encoding/json"
	"fmt"
	"io"
	"time"
)

// Event is one line of a journal. Fields the runner does not emit for a given
// event stay at their zero value.
type Event struct {
	TS         time.Time `json:"ts"`
	Wake       string    `json:"wake"`
	Event      string    `json:"event"`
	Issue      *int64    `json:"issue"`
	Tier       string    `json:"tier"`
	Role       string    `json:"role"`
	Round      int       `json:"round"`
	Harness    string    `json:"harness"`
	Model      string    `json:"model"`
	Lens       string    `json:"lens"`
	Classify   string    `json:"classify"`
	CostUSD    *float64  `json:"cost_usd"`
	CostSource string    `json:"cost_source"`
	DurationS  int64     `json:"duration_s"`
	Verdict    string    `json:"verdict"`
	Reason     string    `json:"reason"`
	Result     string    `json:"result"`
	Failed     string    `json:"failed"`
	Outcome    string    `json:"outcome"`
}

// ParseJournal decodes a JSONL journal. A truncated final line is discarded;
// a parse error on any earlier line is a real error.
func ParseJournal(r io.Reader) ([]Event, error) {
	var events []Event
	lines := []string{}
	sc := bufio.NewScanner(r)
	for sc.Scan() {
		lines = append(lines, sc.Text())
	}
	if err := sc.Err(); err != nil {
		return nil, err
	}
	for i, line := range lines {
		if len(line) == 0 {
			continue
		}
		var ev Event
		if err := json.Unmarshal([]byte(line), &ev); err != nil {
			if i == len(lines)-1 {
				continue // a truncated tail must not hide the events before it
			}
			return nil, fmt.Errorf("journal line %d: %w", i+1, err)
		}
		events = append(events, ev)
	}
	return events, nil
}

// Orphan is a role that started and never reported back.
type Orphan struct {
	Wake    string
	Role    string
	Round   int
	Lens    string
	Tier    string
	Started time.Time
	Age     time.Duration
}

type roleKey struct {
	wake  string
	role  string
	round int
	lens  string
}

// Orphans finds every role_start with no matching role_end sharing its wake,
// role, round and lens.
func Orphans(evs []Event, now time.Time) []Orphan {
	started := map[roleKey]Event{}
	ended := map[roleKey]bool{}
	for _, ev := range evs {
		k := roleKey{ev.Wake, ev.Role, ev.Round, ev.Lens}
		switch ev.Event {
		case "role_start":
			started[k] = ev
		case "role_end":
			ended[k] = true
		}
	}
	var out []Orphan
	for k, ev := range started {
		if ended[k] {
			continue
		}
		age := time.Duration(0)
		if !ev.TS.IsZero() && now.After(ev.TS) {
			age = now.Sub(ev.TS)
		}
		out = append(out, Orphan{
			Wake:    k.wake,
			Role:    k.role,
			Round:   k.round,
			Lens:    k.lens,
			Tier:    ev.Tier,
			Started: ev.TS,
			Age:     age,
		})
	}
	return out
}

// TierCost keeps reported and unknown costs apart, never summed into one figure.
type TierCost struct {
	Reported     float64
	UnknownCount int
}

// CostByTier aggregates role_end costs by tier, by provenance.
func CostByTier(evs []Event) map[string]TierCost {
	out := map[string]TierCost{}
	for _, ev := range evs {
		if ev.Event != "role_end" {
			continue
		}
		c := out[ev.Tier]
		switch ev.CostSource {
		case "reported":
			if ev.CostUSD != nil {
				c.Reported += *ev.CostUSD
			}
		default:
			c.UnknownCount++
		}
		out[ev.Tier] = c
	}
	return out
}
