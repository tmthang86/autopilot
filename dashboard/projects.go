package main

import (
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
	"strings"
)

// Project is one installed autopilot project.
type Project struct {
	Label    string
	Path     string
	Loaded   bool
	Runs     int
	LastExit int
	Err      string
}

// projectArg extracts --project <path> from a plist's ProgramArguments. The
// plist is machine-generated from this repository's own template, so a plain
// string scan is enough and keeps the parser honest about its input.
func projectArg(path string) (string, error) {
	raw, err := os.ReadFile(path)
	if err != nil {
		return "", err
	}
	text := string(raw)
	idx := strings.Index(text, "--project")
	if idx < 0 {
		return "", fmt.Errorf("no --project in %s", path)
	}
	open := strings.Index(text[idx:], "<string>")
	if open < 0 {
		return "", fmt.Errorf("no path string after --project in %s", path)
	}
	start := idx + open + len("<string>")
	end := strings.Index(text[start:], "</string>")
	if end < 0 {
		return "", fmt.Errorf("unterminated path string in %s", path)
	}
	return text[start : start+end], nil
}

// Projects enumerates installed projects from the job plist directory.
func Projects(jobsDir string) []Project {
	entries, err := os.ReadDir(jobsDir)
	if err != nil {
		return nil
	}
	var out []Project
	for _, e := range entries {
		if e.IsDir() || !strings.HasSuffix(e.Name(), ".plist") {
			continue
		}
		path := filepath.Join(jobsDir, e.Name())
		label := strings.TrimSuffix(e.Name(), ".plist")
		p := Project{Label: label}
		proj, err := projectArg(path)
		if err != nil {
			p.Err = err.Error()
			out = append(out, p)
			continue
		}
		p.Path = proj
		p.Loaded, p.Runs, p.LastExit = launchctlState(label)
		out = append(out, p)
	}
	return out
}

func launchctlState(label string) (bool, int, int) {
	out, err := exec.Command("launchctl", "print", "gui/"+uid()+"/"+label).Output()
	if err != nil {
		return false, 0, 0
	}
	text := string(out)
	loaded := !strings.Contains(text, "Could not find service")
	runs := parseField(text, "runs = ")
	lastExit := parseField(text, "last exit code = ")
	return loaded, runs, lastExit
}

func parseField(text, key string) int {
	idx := strings.Index(text, key)
	if idx < 0 {
		return 0
	}
	rest := text[idx+len(key):]
	if i := strings.IndexByte(rest, '\n'); i >= 0 {
		rest = rest[:i]
	}
	n, _ := strconv.Atoi(strings.TrimSpace(rest))
	return n
}

func uid() string {
	return strconv.Itoa(os.Getuid())
}
