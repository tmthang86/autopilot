#!/bin/sh
. "$(dirname "$0")/harness.sh"

# deploy.sh must never consult the operator's real home directory from a test.
dest="$TEST_TMP/deploy"
export AUTOPILOT_DEST="$dest"

run() { sh "$RUNNER_ROOT/deploy.sh" >/dev/null 2>"$TEST_TMP/err"; }

# --- a fresh deploy copies the runner set, and nothing else ---
run; rc=$?
assert_eq "0" "$rc" "a fresh deploy succeeds"
assert_eq "1" "$([ -f "$dest/VERSION" ] && echo 1 || echo 0)" "VERSION lands in the deployment"
assert_eq "1" "$([ -f "$dest/run-once.sh" ] && echo 1 || echo 0)" "run-once.sh lands in the deployment"
assert_eq "1" "$([ -f "$dest/lib/queue.sh" ] && echo 1 || echo 0)" "lib/ lands in the deployment"
assert_eq "1" "$([ -f "$dest/templates/prompt.tmpl" ] && echo 1 || echo 0)" "templates/ lands in the deployment"
assert_eq "0" "$([ -f "$dest/deploy.sh" ] && echo 1 || echo 0)" "deploy.sh itself is not deployed"
assert_eq "0" "$([ -f "$dest/.git" ] && echo 1 || echo 0)" "no git metadata is deployed"

# --- a destination that is a git checkout is refused and untouched ---
rm -rf "$dest"
mkdir -p "$dest/.git"
sentinel="$dest/precious.txt"
echo keep > "$sentinel"
run; rc=$?
assert_eq "1" "$rc" "a destination containing .git is refused"
assert_contains "$(cat "$TEST_TMP/err")" "contains a .git directory" "the refusal names the .git directory"
assert_eq "keep" "$(cat "$sentinel")" "the checkout is left untouched"

# --- a stale deployed copy is re-copied with the current version ---
rm -rf "$dest"
mkdir -p "$dest"
printf '0.0.0\n' > "$dest/VERSION"
run; rc=$?
assert_eq "0" "$rc" "a stale copy is redeployed without complaint"
assert_eq "$(cat "$RUNNER_ROOT/VERSION")" "$(cat "$dest/VERSION")" "the deployed version matches the source"

finish