#!/bin/sh
. "$(dirname "$0")/harness.sh"
. "$RUNNER_ROOT/lib/log.sh"
. "$RUNNER_ROOT/lib/state.sh"

f="$TEST_TMP/state.json"

state_init "$f"
assert_eq "0" "$(state_get resume_after -1)" "init writes defaults"

state_set_num resume_after 1755000000
assert_eq "1755000000" "$(state_get resume_after -1)" "numeric set then get"

state_set tasks_today_date "2026-08-14"
assert_eq "2026-08-14" "$(state_get tasks_today_date x)" "string set then get"

assert_eq "1" "$(state_bump tasks_today)" "bump returns the new value"
assert_eq "2" "$(state_bump tasks_today)" "bump accumulates"

# The invariant: a fresh process must see exactly what the last one wrote.
AUTOPILOT_STATE_FILE=""
state_init "$f"
assert_eq "1755000000" "$(state_get resume_after -1)" "round-trip: numbers survive re-init"
assert_eq "2026-08-14" "$(state_get tasks_today_date x)" "round-trip: strings survive re-init"
assert_eq "2" "$(state_get tasks_today -1)" "round-trip: counters survive re-init"

# init must not clobber an existing file.
state_init "$f"
assert_eq "2" "$(state_get tasks_today -1)" "init is idempotent, never resets"

# A zero must survive as a zero, not be mistaken for absent and replaced by the
# caller's default — the difference between "no backoff" and "unknown".
state_set_num backoff_step 0
assert_eq "0" "$(state_get backoff_step 99)" "a stored zero is not confused with absent"

# A corrupted state file must be replaced, not crash the run.
printf 'garbage' > "$f"
state_init "$f" 2>/dev/null
assert_eq "0" "$(state_get tasks_today -1)" "corrupt state is rebuilt from defaults"

# A parent directory that does not exist yet must be created, because the first
# ever run has no .autopilot/ directory.
deep="$TEST_TMP/nested/dir/state.json"
state_init "$deep"
assert_eq "0" "$(state_get tasks_today -1)" "init creates missing parent directories"

finish
