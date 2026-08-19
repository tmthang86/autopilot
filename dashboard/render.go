package main

import (
	"embed"
	"html/template"
	"sort"
	"strings"
	"time"
)

//go:embed ui
var uiFS embed.FS

// ProjectView is one project as the overview renders it.
type ProjectView struct {
	Label    string
	Path     string
	Loaded   bool
	Runs     int
	LastExit int
	Err      string
	STOP     bool
	Orphans  []Orphan
	Cost     map[string]TierCost
	InFlight int
}

func renderOverview(projects []ProjectView, now time.Time) (string, error) {
	tmpl, err := template.ParseFS(uiFS, "ui/overview.html")
	if err != nil {
		return "", err
	}
	data := struct {
		Projects []ProjectView
		Orphans  []Orphan
		Cost     map[string]TierCost
		Now      time.Time
	}{projects, nil, nil, now}
	var all []Orphan
	for _, p := range projects {
		all = append(all, p.Orphans...)
		data.Cost = mergeCost(data.Cost, p.Cost)
	}
	sort.Slice(all, func(i, j int) bool { return all[i].Age > all[j].Age })
	data.Orphans = all
	var b strings.Builder
	err = tmpl.Execute(&b, data)
	return b.String(), err
}

func mergeCost(a, b map[string]TierCost) map[string]TierCost {
	if a == nil {
		a = map[string]TierCost{}
	}
	for k, v := range b {
		c := a[k]
		c.Reported += v.Reported
		c.UnknownCount += v.UnknownCount
		a[k] = c
	}
	return a
}

func renderTask(events []Event) (string, error) {
	tmpl, err := template.ParseFS(uiFS, "ui/task.html")
	if err != nil {
		return "", err
	}
	var b strings.Builder
	err = tmpl.Execute(&b, events)
	return b.String(), err
}
