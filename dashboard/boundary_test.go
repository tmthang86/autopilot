package main

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func goFiles(root string) []struct{ path, body string } {
	var out []struct{ path, body string }
	_ = filepath.Walk(root, func(p string, info os.FileInfo, err error) error {
		if err != nil || info.IsDir() || !strings.HasSuffix(p, ".go") {
			return nil
		}
		b, err := os.ReadFile(p)
		if err == nil {
			out = append(out, struct{ path, body string }{p, string(b)})
		}
		return nil
	})
	return out
}

func TestTheDashboardNeverWrites(t *testing.T) {
	for _, f := range goFiles(".") {
		if strings.HasSuffix(f.path, "_test.go") {
			continue
		}
		for _, bad := range []string{"os.Create", "os.WriteFile", "os.Remove", "os.OpenFile"} {
			if strings.Contains(f.body, bad) {
				t.Errorf("%s writes: %s", f.path, bad)
			}
		}
	}
}

func TestNoPostRoutes(t *testing.T) {
	for _, f := range goFiles(".") {
		if strings.HasSuffix(f.path, "_test.go") {
			continue
		}
		if strings.Contains(f.body, "MethodPost") {
			t.Errorf("%s accepts POST", f.path)
		}
	}
}
