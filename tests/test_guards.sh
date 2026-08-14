#!/bin/sh
. "$(dirname "$0")/harness.sh"
. "$REPO_ROOT/lib/log.sh"
. "$REPO_ROOT/lib/config.sh"
. "$REPO_ROOT/lib/state.sh"
. "$REPO_ROOT/lib/guards.sh"

repo=$(make_repo)
mkdir -p "$repo/.autopilot"
cp "$REPO_ROOT/templates/config.json" "$repo/.autopilot/config.json"
cfg_load "$repo/.autopilot/config.json"
state_init "$repo/.autopilot/state.json"

# --- STOP file ---
if guard_stop_file "$repo/.autopilot"; then rc=0; else rc=1; fi
assert_eq "0" "$rc" "no STOP file means proceed"
touch "$repo/.autopilot/STOP"
if guard_stop_file "$repo/.autopilot" 2>/dev/null; then rc=0; else rc=1; fi
assert_eq "1" "$rc" "STOP file halts the run"
rm "$repo/.autopilot/STOP"

# --- lock ---
if guard_lock "$repo/.autopilot"; then rc=0; else rc=1; fi
assert_eq "0" "$rc" "first lock acquisition succeeds"
if guard_lock "$repo/.autopilot" 2>/dev/null; then rc=0; else rc=1; fi
assert_eq "1" "$rc" "second acquisition is refused while held"
guard_unlock "$repo/.autopilot"
if guard_lock "$repo/.autopilot"; then rc=0; else rc=1; fi
assert_eq "0" "$rc" "lock is reacquirable after release"
guard_unlock "$repo/.autopilot"

# A lock left by a dead process must not wedge the loop forever. There is no
# operator watching to clear it.
mkdir -p "$repo/.autopilot/lock"
printf '999999' > "$repo/.autopilot/lock/pid"
if guard_lock "$repo/.autopilot" 2>/dev/null; then rc=0; else rc=1; fi
assert_eq "0" "$rc" "stale lock from a dead pid is broken"
guard_unlock "$repo/.autopilot"

# --- resume_after ---
state_set_num resume_after 0
if guard_resume_after; then rc=0; else rc=1; fi
assert_eq "0" "$rc" "zero resume_after means proceed"
state_set_num resume_after "$(( $(date +%s) + 3600 ))"
if guard_resume_after 2>/dev/null; then rc=0; else rc=1; fi
assert_eq "1" "$rc" "future resume_after halts the run"
state_set_num resume_after "$(( $(date +%s) - 60 ))"
if guard_resume_after; then rc=0; else rc=1; fi
assert_eq "0" "$rc" "past resume_after means proceed"

# --- daily cap ---
state_set tasks_today_date "$(date -u +%Y-%m-%d)"
state_set_num tasks_today 11
if guard_daily_cap; then rc=0; else rc=1; fi
assert_eq "0" "$rc" "below the cap means proceed"
state_set_num tasks_today 12
if guard_daily_cap 2>/dev/null; then rc=0; else rc=1; fi
assert_eq "1" "$rc" "at the cap halts the run"

# The counter must reset when the date rolls over, or the loop dies after one day.
state_set tasks_today_date "2020-01-01"
if guard_daily_cap; then rc=0; else rc=1; fi
assert_eq "0" "$rc" "a new day resets the counter"
assert_eq "0" "$(state_get tasks_today -1)" "counter is actually zeroed on rollover"

# --- quiet hours ---
# The clock is injected, so these assertions hold at every hour of the day.
cfgq() { jq ".pacing.quiet_hours = $1" "$REPO_ROOT/templates/config.json" > "$repo/.autopilot/config.json"; cfg_load "$repo/.autopilot/config.json"; }
quiet_at() { AUTOPILOT_NOW_HM=$1 guard_quiet_hours 2>/dev/null && printf 'allow' || printf 'halt'; }

cfgq '[]'
assert_eq "allow" "$(quiet_at 1200)" "no quiet hours means proceed"

# Non-wrapping window: 09:00–18:00, the "leave my working day alone" case.
cfgq '["09:00-18:00"]'
assert_eq "halt"  "$(quiet_at 1200)" "midday is inside a 09:00-18:00 window"
assert_eq "halt"  "$(quiet_at 0900)" "the opening minute is inside the window"
assert_eq "halt"  "$(quiet_at 1800)" "the closing minute is inside the window"
assert_eq "allow" "$(quiet_at 0859)" "one minute before the window is outside"
assert_eq "allow" "$(quiet_at 1801)" "one minute after the window is outside"
assert_eq "allow" "$(quiet_at 0300)" "the small hours are outside a daytime window"

# Wrapping window: 23:00–07:00. Naive from<=now<=to gets this exactly backwards,
# halting all day and running all night — the opposite of what was configured.
cfgq '["23:00-07:00"]'
assert_eq "halt"  "$(quiet_at 2330)" "late evening is inside a wrapping window"
assert_eq "halt"  "$(quiet_at 0200)" "after midnight is still inside it"
assert_eq "halt"  "$(quiet_at 0700)" "the closing minute is inside it"
assert_eq "allow" "$(quiet_at 1200)" "midday is outside a wrapping window"
assert_eq "allow" "$(quiet_at 2259)" "one minute before it opens is outside"

# Leading zeros must not be read as octal: 08 and 09 are invalid octal and
# would abort the shell's arithmetic mid-comparison.
cfgq '["08:00-09:00"]'
assert_eq "halt"  "$(quiet_at 0830)" "08:xx and 09:xx compare correctly, not as octal"

# --- project present ---
cfgq '[]'
if guard_project_present "$repo"; then rc=0; else rc=1; fi
assert_eq "0" "$rc" "existing project root means proceed"
if guard_project_present "/nonexistent/volume/project" 2>/dev/null; then rc=0; else rc=1; fi
assert_eq "1" "$rc" "missing project root halts the run"

finish
