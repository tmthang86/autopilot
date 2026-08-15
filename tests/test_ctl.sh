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

finish
