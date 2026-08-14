#!/bin/sh
. "$(dirname "$0")/harness.sh"
. "$REPO_ROOT/lib/log.sh"
. "$REPO_ROOT/lib/config.sh"
. "$REPO_ROOT/lib/verify.sh"

repo=$(make_repo)
mkdir -p "$repo/.autopilot"

mkcfg() {
    jq ".verify = $1" "$REPO_ROOT/templates/config.json" > "$repo/.autopilot/config.json"
    cfg_load "$repo/.autopilot/config.json"
}

mkcfg '[{"name":"a","cmd":"true"},{"name":"b","cmd":"true"}]'
if verify_run "$repo"; then rc=0; else rc=1; fi
assert_eq "0" "$rc" "all commands passing returns success"

mkcfg '[{"name":"a","cmd":"true"},{"name":"b","cmd":"echo boom >&2; false"},{"name":"c","cmd":"echo ran-c > '"$repo"'/c.marker"}]'
rm -f "$repo/c.marker"
if verify_run "$repo" 2>/dev/null; then rc=0; else rc=1; fi
assert_eq "1" "$rc" "one failing command fails the whole gate"
assert_eq "b" "$VERIFY_FAILED_NAME" "the failing command is named"
assert_contains "$VERIFY_OUTPUT" "boom" "the failure output is captured"
assert_eq "0" "$([ -f "$repo/c.marker" ] && echo 1 || echo 0)" "commands after the failure do not run"

# An empty verify list must NOT pass. A project with no checks has no oracle,
# and silently allowing commits would defeat the entire gate while appearing to
# honour it.
mkcfg '[]'
if verify_run "$repo" 2>/dev/null; then rc=0; else rc=1; fi
assert_eq "1" "$rc" "an empty verify list is refused, not treated as success"

# Commands must run in the project, not wherever the runner happens to be.
mkcfg '[{"name":"cwd","cmd":"test -f seed.txt"}]'
if verify_run "$repo"; then rc=0; else rc=1; fi
assert_eq "0" "$rc" "commands run inside the project root"

# State from a previous failure must not leak into the next run's report.
mkcfg '[{"name":"ok","cmd":"true"}]'
verify_run "$repo" >/dev/null 2>&1
assert_eq "" "$VERIFY_FAILED_NAME" "a passing run clears the previous failure name"
assert_eq "" "$VERIFY_OUTPUT" "a passing run clears the previous failure output"

finish
