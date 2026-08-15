#!/bin/sh
. "$(dirname "$0")/harness.sh"
. "$RUNNER_ROOT/lib/log.sh"
. "$RUNNER_ROOT/lib/config.sh"

good="$TEST_TMP/good.json"
cp "$RUNNER_ROOT/templates/config.json" "$good"

if cfg_load "$good"; then rc=0; else rc=1; fi
assert_eq "0" "$rc" "cfg_load accepts the template"

assert_eq "sonnet" "$(cfg_get agent.default_model x)" "cfg_get reads a nested value"
assert_eq "fallback" "$(cfg_get agent.nonexistent fallback)" "cfg_get returns the default when absent"
assert_eq "main" "$(cfg_get project.main_branch x)" "cfg_get reads main_branch"

count=$(cfg_list queue.exclude_labels | wc -l | tr -d ' ')
assert_eq "3" "$count" "cfg_list emits one line per element"
assert_contains "$(cfg_list queue.exclude_labels)" "needs-human" "cfg_list content is correct"

# An absent list must yield nothing rather than an error, so callers can loop
# over it unconditionally.
assert_eq "" "$(cfg_list pacing.quiet_hours)" "an empty list yields nothing"

# Malformed JSON must be rejected, not silently treated as empty.
bad="$TEST_TMP/bad.json"
printf '{ not json' > "$bad"
if cfg_load "$bad" 2>/dev/null; then rc=0; else rc=1; fi
assert_eq "1" "$rc" "cfg_load rejects malformed JSON"

# A missing required key must be rejected, so misconfiguration fails at the
# guard stage rather than halfway through a task.
missing="$TEST_TMP/missing.json"
jq 'del(.verify)' "$good" > "$missing"
if cfg_load "$missing" 2>/dev/null; then rc=0; else rc=1; fi
assert_eq "1" "$rc" "cfg_load rejects config with no verify block"

# A nonexistent path is an operator error and must be loud.
if cfg_load "$TEST_TMP/nope.json" 2>/dev/null; then rc=0; else rc=1; fi
assert_eq "1" "$rc" "cfg_load rejects a missing file"

# A rejected load must not leave the previous config selected, or the runner
# would silently keep using stale settings.
cfg_load "$good" >/dev/null 2>&1
cfg_load "$bad"  >/dev/null 2>&1
assert_eq "" "$(cfg_get project.main_branch '')" "a failed load clears the selected config"

finish
