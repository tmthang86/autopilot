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
# The task's intent pointer must resolve on the work branch too, so the
# document it names is committed before the branch splits.
mkdir -p "$repo/docs/plans"
echo "the approved plan" > "$repo/docs/plans/0001.md"
git -C "$repo" add docs/plans/0001.md
git -C "$repo" commit -q -m "add the approved plan"
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
[{"number":5,"title":"Do a thing","body":"Intent: docs/plans/0001.md\nCreate marker.txt","labels":[{"name":"autopilot"},{"name":"model:opus"},{"name":"effort:high"}],"milestone":null}]
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

# --- gh refusing `issue edit` is the claim/release failure, and only running
# the real script can see it ---
# queue_claim and queue_release were `gh issue edit ... >/dev/null 2>&1`
# invoked as bare statements. Under run-once.sh's `set -eu` a failing gh then
# killed the run where it stood, with its stderr already thrown away: no log
# line, no exit path, nothing for the operator to read. tests/harness.sh never
# sets -e, so a unit test sourcing queue.sh passes either way — these two
# blocks run run-once.sh itself.
#
# One stub serves both: it answers `issue list` with a real issue and, through
# GH_FAIL_EDIT, fails exactly one kind of `gh issue edit` — the claim
# (`--add-label`) or the release (`--remove-label`) — the way a 5xx or an
# expired token does.
stub_bin gh '_fail() { printf "HTTP 503: gh issue edit failed (api.github.com)\n" >&2; exit 1; }
printf "%s\n" "$*" >> "$GH_CALLS"
case "$1 $2" in
  "issue list") cat <<JSON
[{"number":5,"title":"Do a thing","body":"Intent: docs/plans/0001.md\nCreate marker2.txt","labels":[{"name":"autopilot"}],"milestone":null}]
JSON
    ;;
  "issue edit")
    case "${GH_FAIL_EDIT:-none} $*" in
      "claim "*--add-label*)      _fail ;;
      "release "*--remove-label*) _fail ;;
    esac
    printf "{}\n"
    ;;
  *) printf "{}\n" ;;
esac'
export GH_FAIL_EDIT
AGENT_RAN="$TEST_TMP/agent-ran"; export AGENT_RAN
stub_bin claude 'echo ran >> "$AGENT_RAN"; echo "{\"type\":\"result\",\"is_error\":false}"; echo done > marker2.txt; exit 0'

reset_run_state() {
    jq '.verify = [{"name":"t","cmd":"true"}]' "$RUNNER_ROOT/templates/config.json" > "$repo/.autopilot/config.json"
    jq '.resume_after = 0 | .consecutive_failures = 0' "$repo/.autopilot/state.json" > "$TEST_TMP/s" \
        && mv "$TEST_TMP/s" "$repo/.autopilot/state.json"
    LOG_DIR="$repo/.autopilot/logs"; rm -f "$LOG_DIR"/*.log
    rm -f "$AGENT_RAN"
    : > "$GH_CALLS"
}

# A claim that did not take must stop the run before an agent turn is spent on
# an issue this run does not hold.
reset_run_state
GH_FAIL_EDIT=claim
tasks_before=$(jq -r .tasks_today "$repo/.autopilot/state.json")
run; rc=$?
log=$(cat "$LOG_DIR"/*.log 2>/dev/null)
assert_eq "1" "$rc" "a claim gh refuses exits non-zero instead of dying mid-script"
assert_contains "$log" "could not claim #5" "the log names the issue that could not be claimed"
assert_contains "$log" "HTTP 503" "gh's own stderr reaches the log rather than /dev/null"
assert_eq "0" "$([ -f "$AGENT_RAN" ] && echo 1 || echo 0)" "no agent turn is spent on an issue that was not claimed"
assert_eq "$tasks_before" "$(jq -r .tasks_today "$repo/.autopilot/state.json")" \
    "a run that never started does not count against the daily cap"

# A release that did not take happens after the work is already pushed. It must
# never discard that: the commit is delivered, the issue is still settled, and
# the leftover claim label is reported so a person can remove it.
reset_run_state
GH_FAIL_EDIT=release
run; rc=$?
log=$(cat "$LOG_DIR"/*.log 2>/dev/null)
assert_eq "0" "$rc" "a release failure after a green verify is not reported as a crashed run"
assert_eq "$(git -C "$repo" rev-parse HEAD)" "$(git -C "$TEST_TMP/tester-proj.git" rev-parse autopilot/main)" \
    "the work still reached the remote"
assert_contains "$log" "could not release #5" "the log names the release that failed"
assert_contains "$log" "HTTP 503" "gh's own stderr reaches the log on the release path too"
assert_contains "$log" "status:in-progress" "the log says which label was left behind"
# This one is _settle_release's own wrapper line (runner/lib/settle.sh), not
# queue_release's — queue.sh's message alone already satisfies the three
# assertions above, so deleting settle.sh's wrapper line would leave them all
# green. "an excluded label" only appears in the wrapper.
assert_contains "$log" "an excluded label" "settle.sh's own wrapper line explains why the leftover label matters"
assert_contains "$(cat "$GH_CALLS")" "issue close 5" "the run settles the issue rather than dying at the failed release"
GH_FAIL_EDIT=none

# --- a task naming no intent is refused before anything is spent on it ---
# Intent is resolved before queue_claim, so the refusal must cost no token, no
# claim label, no daily-cap increment, and no circuit-breaker increment. The
# issue is commented and labelled blocked so a person knows what to fix.
reset_run_state
stub_bin gh 'printf "%s\n" "$*" >> "$GH_CALLS"
case "$1 $2" in
  "issue list") cat <<JSON
[{"number":6,"title":"No intent","body":"Just some work","labels":[{"name":"autopilot"}],"milestone":null}]
JSON
    ;;
  *) printf "{}\n" ;;
esac'
stub_bin claude 'echo "{\"type\":\"result\",\"is_error\":false}"'
: > "$GH_CALLS"
tasks_before=$(jq -r .tasks_today "$repo/.autopilot/state.json")
fails_before=$(jq -r .consecutive_failures "$repo/.autopilot/state.json")
run; rc=$?
log=$(cat "$LOG_DIR"/*.log 2>/dev/null)
assert_eq "0" "$rc" "a run refusing a pointerless issue exits 0, not as a crash"
assert_contains "$log" "names no intent" "the log names the missing intent"
assert_contains "$(cat "$GH_CALLS")" "issue comment 6" "the refusal is commented on the issue"
assert_contains "$(cat "$GH_CALLS")" "--add-label blocked" "the issue is labelled blocked"
assert_eq "0" "$([ -f "$AGENT_RAN" ] && echo 1 || echo 0)" "no agent turn is spent on a refused issue"
assert_eq "$tasks_before" "$(jq -r .tasks_today "$repo/.autopilot/state.json")" \
    "a refused issue does not count against the daily cap"
assert_eq "$fails_before" "$(jq -r .consecutive_failures "$repo/.autopilot/state.json")" \
    "a refused issue does not touch the circuit breaker"
assert_eq "0" "$(grep -c 'issue edit 6 --add-label status:in-progress' "$GH_CALLS" || true)" \
    "a refused issue is never claimed"

# --- a pointer escaping the project root is refused the same way ---
reset_run_state
stub_bin gh 'printf "%s\n" "$*" >> "$GH_CALLS"
case "$1 $2" in
  "issue list") cat <<JSON
[{"number":7,"title":"Escaping","body":"Intent: ../outside.md\nWork","labels":[{"name":"autopilot"}],"milestone":null}]
JSON
    ;;
  *) printf "{}\n" ;;
esac'
stub_bin claude 'echo "{\"type\":\"result\",\"is_error\":false}"'
: > "$GH_CALLS"
tasks_before=$(jq -r .tasks_today "$repo/.autopilot/state.json")
run; rc=$?
log=$(cat "$LOG_DIR"/*.log 2>/dev/null)
assert_eq "0" "$rc" "a run refusing an escaping pointer exits 0"
assert_contains "$log" "traverses upward" "the log names the containment failure specifically"
assert_contains "$(cat "$GH_CALLS")" "issue comment 7" "the escaping pointer is also commented on"
assert_eq "0" "$([ -f "$AGENT_RAN" ] && echo 1 || echo 0)" "no agent turn is spent on an escaping pointer"
assert_eq "$tasks_before" "$(jq -r .tasks_today "$repo/.autopilot/state.json")" \
    "an escaping pointer does not count against the daily cap"

# Skills run when a person is present; the loop runs when nobody is. A loop
# that could invoke a skill would reach the one capability reserved for
# supervised moments.
assert_eq "" "$(grep -rl 'skills/' "$RUNNER_ROOT/run-once.sh" "$RUNNER_ROOT/lib/" 2>/dev/null)" \
    "the unattended loop never reaches into the skill layer"

finish
