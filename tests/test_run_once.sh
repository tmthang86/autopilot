#!/bin/sh
. "$(dirname "$0")/harness.sh"

repo=$(make_repo)
# A real project has an origin the runner can both parse and push to. The slug
# comes from the URL so gh is never left guessing from the caller's working
# directory, and the push has to actually succeed — settling refuses to close an
# issue whose work has not reached the remote.
git init -q --bare "$TEST_TMP/tester-proj.git"
git -C "$repo" remote add origin "$TEST_TMP/tester-proj.git"
git -C "$repo" push -q origin main
mkdir -p "$repo/.autopilot"
jq '.verify = [{"name":"t","cmd":"true"}]' "$RUNNER_ROOT/templates/config.json" > "$repo/.autopilot/config.json"
git -C "$repo" checkout -q -b autopilot/main
git -C "$repo" checkout -q main

GH_CALLS="$TEST_TMP/calls.txt"; export GH_CALLS; : > "$GH_CALLS"
run() { sh "$RUNNER_ROOT/run-once.sh" --project "$repo" >/dev/null 2>&1; }

# --- STOP must short-circuit before anything else happens ---
touch "$repo/.autopilot/STOP"
stub_bin gh 'echo "$*" >> "$GH_CALLS"; exit 0'
run; rc=$?
assert_eq "0" "$rc" "STOP file yields a clean exit"
assert_eq "" "$(cat "$GH_CALLS")" "STOP file prevents any queue call"
rm "$repo/.autopilot/STOP"

# --- an empty queue is a normal no-op, and must release the lock ---
stub_bin gh 'case "$*" in *"issue list"*) echo "[]" ;; *) echo "{}" ;; esac'
run; rc=$?
assert_eq "0" "$rc" "empty queue yields a clean exit"
assert_eq "0" "$([ -d "$repo/.autopilot/lock" ] && echo 1 || echo 0)" "the lock is released on the empty-queue path"

# --- a genuine gh failure while checking the ready label must not crash the
# runner. run-once.sh runs under `set -eu`; a bare `_names=$(gh ...)`
# assignment inside queue.sh aborts the whole script the instant `gh` fails,
# with no log line at all, even nested inside an `if` body. This can only be
# caught by running the real script -- tests/harness.sh never sets -e, so a
# unit test sourcing queue.sh directly would pass whether or not the guard
# is there. ---
: > "$GH_CALLS"
LOG_DIR="$repo/.autopilot/logs"; rm -f "$LOG_DIR"/*.log
stub_bin gh 'case "$1 $2" in
  "issue list") echo "[]" ;;
  "label list") echo "boom: cannot list labels" >&2; exit 1 ;;
  *) echo "{}" ;;
esac'
run; rc=$?
assert_eq "0" "$rc" "a gh failure while checking the ready label still exits 0, not a crash"
assert_contains "$(cat "$LOG_DIR"/*.log 2>/dev/null)" "boom: cannot list labels" \
    "the log names the real gh failure rather than being empty"

# --- a missing config is an operator error and must be loud ---
bare=$(make_repo)
sh "$RUNNER_ROOT/run-once.sh" --project "$bare" >/dev/null 2>&1; rc=$?
assert_eq "1" "$rc" "missing config exits non-zero"

# --- a full successful run ---
stub_bin gh 'case "$*" in
  *"issue list"*) cat <<JSON
[{"number":5,"title":"Do a thing","body":"Create marker.txt","labels":[{"name":"autopilot"},{"name":"model:opus"},{"name":"effort:high"}],"milestone":null}]
JSON
  ;;
  *) echo "$*" >> "$GH_CALLS"; echo "{}" ;;
esac'
# The stub agent stands in for Claude: it does the work the issue describes.
stub_bin claude 'echo "{\"type\":\"result\",\"is_error\":false}"; echo done > marker.txt; exit 0'
: > "$GH_CALLS"
run; rc=$?
assert_eq "0" "$rc" "a successful run exits 0"
assert_contains "$(git -C "$repo" log -1 --pretty=%B)" "Closes #5" "the work is committed with a closing trailer"
assert_contains "$(cat "$GH_CALLS")" "issue close 5" "the issue is closed"
assert_eq "autopilot/main" "$(git -C "$repo" rev-parse --abbrev-ref HEAD)" "work happened on the work branch"
assert_eq "1" "$(jq -r .tasks_today "$repo/.autopilot/state.json")" "the daily counter advanced"

# --- usage exhaustion must not consume the task's retry budget (invariant 2) ---
stub_bin claude 'exit 1'
: > "$GH_CALLS"
before_failures=$(jq -r .consecutive_failures "$repo/.autopilot/state.json")
run; rc=$?
assert_eq "0" "$rc" "a closed usage window exits 0, not as a failure"
after=$(jq -r .consecutive_failures "$repo/.autopilot/state.json")
assert_eq "$before_failures" "$after" "usage exhaustion does not count as a task failure"
resume=$(jq -r .resume_after "$repo/.autopilot/state.json")
assert_eq "1" "$([ "$resume" -gt "$(date +%s)" ] && echo 1 || echo 0)" "resume_after is set into the future"
case "$(cat "$GH_CALLS")" in *"issue comment"*) blamed=1 ;; *) blamed=0 ;; esac
assert_eq "0" "$blamed" "the innocent issue is not blamed on the issue thread"

# --- the resume_after just written must make the next wake a no-op ---
: > "$GH_CALLS"
run
assert_eq "" "$(cat "$GH_CALLS")" "the next wake stands down without touching the queue"

# --- a red verify suite must discard the work rather than commit it ---
jq '.verify = [{"name":"impossible","cmd":"false"}]' "$RUNNER_ROOT/templates/config.json" > "$repo/.autopilot/config.json"
jq '.resume_after = 0' "$repo/.autopilot/state.json" > "$TEST_TMP/s" && mv "$TEST_TMP/s" "$repo/.autopilot/state.json"
stub_bin claude 'echo "{\"type\":\"result\",\"is_error\":false}"; echo junk > should-not-land.txt; exit 0'
: > "$GH_CALLS"
before=$(git -C "$repo" rev-parse HEAD)
run
assert_eq "$before" "$(git -C "$repo" rev-parse HEAD)" "a red verify suite creates no commit"
assert_eq "0" "$([ -f "$repo/should-not-land.txt" ] && echo 1 || echo 0)" "the discarded work is gone from the tree"
assert_contains "$(cat "$GH_CALLS")" "issue comment" "the failure is reported on the issue"

finish
