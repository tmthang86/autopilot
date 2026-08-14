#!/bin/sh
. "$(dirname "$0")/harness.sh"

# Never touch the operator's real LaunchAgents directory from a test.
AGENTS="$TEST_TMP/LaunchAgents"
AUTOPILOT_LAUNCHAGENTS_DIR="$AGENTS"
export AUTOPILOT_LAUNCHAGENTS_DIR

LCTL_CALLS="$TEST_TMP/launchctl.txt"; export LCTL_CALLS; : > "$LCTL_CALLS"
stub_bin launchctl 'echo "$*" >> "$LCTL_CALLS"; exit 0'

repo=$(make_repo)
name=$(basename "$repo")

# --- install ---
sh "$REPO_ROOT/install-project.sh" "$repo" 1800 >/dev/null 2>&1
assert_eq "1" "$([ -f "$repo/.autopilot/config.json" ] && echo 1 || echo 0)" "a config is created"
assert_eq "$name" "$(jq -r .project.name "$repo/.autopilot/config.json")" "the project name is substituted"

plist="$AGENTS/com.autopilot.$name.plist"
assert_eq "1" "$([ -f "$plist" ] && echo 1 || echo 0)" "the launchd plist is written"
assert_contains "$(cat "$plist")" "1800"        "the interval is substituted"
assert_contains "$(cat "$plist")" "$repo"       "the project path is substituted"
assert_contains "$(cat "$plist")" "run-once.sh" "the runner path is substituted"
assert_contains "$(cat "$plist")" "/opt/homebrew/bin" "PATH is set explicitly for launchd"

# No RunAtLoad: the loop must not come back on its own after a reboot.
case "$(cat "$plist")" in
    *"<key>RunAtLoad</key>"*) has=1 ;;
    *)                        has=0 ;;
esac
assert_eq "0" "$has" "the plist carries no RunAtLoad"

# Installing must never start the loop. Enabling a program that runs with
# permission checks disabled is the operator's decision, not the installer's.
assert_eq "" "$(cat "$LCTL_CALLS")" "the installer does not load or start the job"

# --- gitignore ---
gi=$(cat "$repo/.gitignore")
assert_contains "$gi" ".autopilot/state.json" "state is gitignored"
assert_contains "$gi" ".autopilot/logs/"      "logs are gitignored"
assert_contains "$gi" ".autopilot/STOP"       "the kill switch is gitignored"

# --- idempotence: re-running must not duplicate or clobber ---
jq '.project.name = "EDITED"' "$repo/.autopilot/config.json" > "$TEST_TMP/c" && mv "$TEST_TMP/c" "$repo/.autopilot/config.json"
sh "$REPO_ROOT/install-project.sh" "$repo" 1800 >/dev/null 2>&1
assert_eq "EDITED" "$(jq -r .project.name "$repo/.autopilot/config.json")" "an existing config is never overwritten"
assert_eq "1" "$(grep -c '^\.autopilot/STOP$' "$repo/.gitignore")" "gitignore entries are not duplicated"

# --- a non-git directory is an operator error ---
plain="$TEST_TMP/plain"; mkdir -p "$plain"
sh "$REPO_ROOT/install-project.sh" "$plain" >/dev/null 2>&1; rc=$?
assert_eq "1" "$rc" "a directory that is not a git repository is refused"

# --- start / stop ---
: > "$LCTL_CALLS"
sh "$REPO_ROOT/ctl.sh" start "$repo" >/dev/null 2>&1
calls=$(cat "$LCTL_CALLS")
assert_contains "$calls" "enable"    "start enables the service first"
assert_contains "$calls" "bootstrap" "start bootstraps the job"
# bootstrap refuses a disabled service, so enable must come first.
assert_eq "0" "$(printf '%s' "$calls" | grep -n 'enable' | cut -d: -f1 | head -1 | awk '{print ($1<=1)?0:1}')" "enable precedes bootstrap"

: > "$LCTL_CALLS"
sh "$REPO_ROOT/ctl.sh" stop "$repo" >/dev/null 2>&1
calls=$(cat "$LCTL_CALLS")
assert_contains "$calls" "bootout" "stop boots the job out"
# bootout alone lasts only until the next login; disable is what persists.
assert_contains "$calls" "disable" "stop disables the service so it stays off across reboots"

finish
