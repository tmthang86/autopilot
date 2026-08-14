#!/bin/sh
. "$(dirname "$0")/harness.sh"
. "$REPO_ROOT/lib/log.sh"
. "$REPO_ROOT/lib/config.sh"
. "$REPO_ROOT/lib/agent.sh"

repo=$(make_repo)
mkdir -p "$repo/.autopilot"
cp "$REPO_ROOT/templates/config.json" "$repo/.autopilot/config.json"
cfg_load "$repo/.autopilot/config.json"

# --- argument construction ---
args=$(agent_args sonnet low)
assert_contains "$args" "--model sonnet"                  "model is passed"
assert_contains "$args" "--effort low"                    "effort is passed"
assert_contains "$args" "--max-budget-usd"                "spend ceiling is passed"
assert_contains "$args" "--dangerously-skip-permissions"  "bypassPermissions maps to the real flag"
assert_contains "$args" "--add-dir"                       "directory scope is passed"
assert_contains "$args" "stream-json"                     "stream output is requested"

# A different permission mode must not silently become the bypass flag.
jq '.agent.permission_mode = "acceptEdits"' "$REPO_ROOT/templates/config.json" > "$repo/.autopilot/config.json"
cfg_load "$repo/.autopilot/config.json"
args=$(agent_args sonnet low)
assert_contains "$args" "--permission-mode acceptEdits" "a non-bypass mode is passed through as itself"
case "$args" in
    *dangerously*) leaked=1 ;;
    *)             leaked=0 ;;
esac
assert_eq "0" "$leaked" "acceptEdits does NOT enable the bypass flag"
cp "$REPO_ROOT/templates/config.json" "$repo/.autopilot/config.json"
cfg_load "$repo/.autopilot/config.json"

# --- the recorded fixture must parse ---
fix="$REPO_ROOT/tests/fixtures/claude-result-success.json"
assert_eq "false" "$(jq -r .is_error "$fix")" "recorded success envelope has is_error false"
assert_eq "result" "$(jq -r .type "$fix")"    "recorded envelope is a result"

log="$TEST_TMP/run.log"

# --- classification ---
printf '{"type":"result","is_error":false,"result":"done"}\n' > "$log"
assert_eq "ok" "$(agent_classify "$log" 0)" "clean result classifies as ok"

# An explicit rate-limit event in the stream is decisive on its own, with no probe.
printf '{"type":"rate_limit_event","rate_limit_info":{"status":"rejected"}}\n' > "$log"
stub_bin claude 'echo PROBE_RAN >> "$PROBE_MARK"; exit 0'
PROBE_MARK="$TEST_TMP/probe.txt"; export PROBE_MARK; : > "$PROBE_MARK"
assert_eq "usage_limit" "$(agent_classify "$log" 1)" "rate_limit_event classifies as usage_limit"
assert_eq "" "$(cat "$PROBE_MARK")" "a decisive stream signal skips the probe entirely"

# A healthy rate_limit_event must not be mistaken for throttling — it is present
# on every successful run.
printf '{"type":"rate_limit_event","rate_limit_info":{"status":"allowed"}}
{"type":"result","is_error":false,"result":"done"}\n' > "$log"
assert_eq "ok" "$(agent_classify "$log" 0)" "an allowed rate_limit_event is not throttling"

# Ambiguous failure, probe succeeds → the task is at fault.
printf '{"type":"result","is_error":true,"result":"compile error"}\n' > "$log"
stub_bin claude 'exit 0'
assert_eq "task_failure" "$(agent_classify "$log" 1)" "probe succeeds, so the failure belongs to the task"

# Ambiguous failure, probe also fails → the account is throttled, task innocent.
# This is invariant 2: an outage must never consume a task's retry budget.
stub_bin claude 'exit 1'
assert_eq "usage_limit" "$(agent_classify "$log" 1)" "probe fails, so the account is throttled"

# A vanished log must not be read as success.
rm -f "$log"
stub_bin claude 'exit 0'
assert_eq "task_failure" "$(agent_classify "$log" 1)" "missing log with nonzero exit is a task failure"
assert_eq "ok" "$(agent_classify "$log" 0)" "missing log with zero exit is still ok"

# --- prompt rendering ---
p=$(agent_prompt 42 "Add a thing" "Body line one." "autopilot/main" "Demo")
assert_contains "$p" "TASK #42"        "the issue number reaches the prompt"
assert_contains "$p" "Add a thing"     "the title reaches the prompt"
assert_contains "$p" "Body line one."  "the body reaches the prompt"
assert_contains "$p" "autopilot/main"  "the work branch reaches the prompt"
assert_contains "$p" "blocked"         "the blocked-rather-than-guess rule is present"

# The invariant prefix must be byte-identical between runs so the provider's
# prompt cache can absorb it; only the task block may differ.
a=$(agent_prompt 1 "A" "x" "b" "P" | sed -n '1,/^--- TASK/p' | sed '$d')
b=$(agent_prompt 2 "B" "y" "b" "P" | sed -n '1,/^--- TASK/p' | sed '$d')
assert_eq "$a" "$b" "the prompt prefix is identical across issues"

# A missing template must fail loudly. Rendering nothing would hand the agent an
# empty task and let it improvise, which is what this design exists to prevent.
saved_home=$AUTOPILOT_HOME
AUTOPILOT_HOME="$TEST_TMP/nowhere"
if agent_prompt 1 "T" "B" "b" "P" >/dev/null 2>&1; then rc=0; else rc=1; fi
assert_eq "1" "$rc" "a missing prompt template fails rather than rendering nothing"
AUTOPILOT_HOME=$saved_home

finish
