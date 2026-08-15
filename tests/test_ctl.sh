#!/bin/sh
. "$(dirname "$0")/harness.sh"

# ctl.sh must never touch the operator's real launchd domain from a test.
AUTOPILOT_PLIST_DIR="$TEST_TMP/jobs"
export AUTOPILOT_PLIST_DIR
mkdir -p "$AUTOPILOT_PLIST_DIR"

# launchctl print reports every job as absent — the "no job loaded" case.
stub_bin launchctl 'exit 1'

repo="$TEST_TMP/proj"
mkdir -p "$repo"
git -C "$repo" init -q

out=$(sh "$RUNNER_ROOT/ctl.sh" status "$repo" 2>&1)

assert_contains "$out" "runner:" "status names the runner version"
assert_contains "$out" "$(cat "$RUNNER_ROOT/VERSION")" "status prints the deployed version"

# --- stop must verify, the way start does ---
# `stop` ran bootout and disable as `2>/dev/null || true` and then printed that
# autopilot was stopped whatever happened. `start` was hardened against exactly
# this and `stop` was not, so a job that survived bootout kept firing every
# interval behind a reassuring sentence and a zero exit status.

# The job really did go: launchctl print reports it absent afterwards.
stub_bin launchctl 'case "$1" in print) exit 1 ;; esac; exit 0'
out=$(sh "$RUNNER_ROOT/ctl.sh" stop "$repo" 2>&1); rc=$?
assert_eq "0" "$rc" "a stop that really stopped the job exits 0"
assert_contains "$out" "stopped" "a real stop still says so"

# The job survived bootout — it is still loaded when launchctl is asked again.
stub_bin launchctl 'case "$1" in
  print) exit 0 ;;
  bootout) printf "Boot-out failed: 5: Input/output error\n" >&2; exit 5 ;;
esac
exit 0'
out=$(sh "$RUNNER_ROOT/ctl.sh" stop "$repo" 2>&1); rc=$?
assert_eq "1" "$rc" "a stop that did not stop the job exits non-zero"
assert_contains "$out" "FAILED to stop" "the operator is told the stop did not happen"
assert_contains "$out" "Input/output error" "launchctl's own message reaches the operator"

# Nothing was loaded under this label at all. That is a clean stop, but it is
# also what an upgraded project looks like when its running job sits under the
# pre-0.1.0 label, so the operator is pointed at the migration note.
stub_bin launchctl 'case "$1" in print) exit 1 ;; esac; exit 0'
out=$(sh "$RUNNER_ROOT/ctl.sh" stop "$repo" 2>&1); rc=$?
assert_eq "0" "$rc" "stopping a job that was not loaded is not an error"
assert_contains "$out" "no job was loaded" "a stop with nothing to stop says so"
assert_contains "$out" "install.md" "the operator is pointed at the orphaned-job note"

finish
