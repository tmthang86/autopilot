#!/bin/sh
. "$(dirname "$0")/harness.sh"
. "$RUNNER_ROOT/lib/log.sh"
. "$RUNNER_ROOT/lib/config.sh"
. "$RUNNER_ROOT/lib/agent.sh"

# tests/run.sh exports AUTOPILOT_HOME; a run of this file on its own must
# still find the real template rather than failing on an unset variable.
AUTOPILOT_HOME=${AUTOPILOT_HOME:-$RUNNER_ROOT}
export AUTOPILOT_HOME

repo=$(make_repo)
mkdir -p "$repo/.autopilot"
cp "$RUNNER_ROOT/templates/config.json" "$repo/.autopilot/config.json"
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
jq '.agent.permission_mode = "acceptEdits"' "$RUNNER_ROOT/templates/config.json" > "$repo/.autopilot/config.json"
cfg_load "$repo/.autopilot/config.json"
args=$(agent_args sonnet low)
assert_contains "$args" "--permission-mode acceptEdits" "a non-bypass mode is passed through as itself"
case "$args" in
    *dangerously*) leaked=1 ;;
    *)             leaked=0 ;;
esac
assert_eq "0" "$leaked" "acceptEdits does NOT enable the bypass flag"
cp "$RUNNER_ROOT/templates/config.json" "$repo/.autopilot/config.json"
cfg_load "$repo/.autopilot/config.json"

# --- the recorded fixture must parse ---
fix="$REPO_ROOT/tests/fixtures/claude-result-success.json"
assert_eq "false" "$(jq -r .is_error "$fix")" "recorded success envelope has is_error false"
assert_eq "result" "$(jq -r .type "$fix")"    "recorded envelope is a result"

# --add-dir is variadic, so it must never be the last flag: a positional prompt
# after it is swallowed as another directory and the CLI reports no input.
last=$(printf '%s' "$args" | awk '{print $NF}')
assert_eq "0" "$([ "$last" = "." ] && echo 1 || echo 0)" "--add-dir is not the final flag"

# The prompt is delivered on stdin, which is immune to flag order and to a task
# description that happens to begin with a dash.
STDIN_SEEN="$TEST_TMP/stdin.txt"; export STDIN_SEEN
stub_bin claude 'cat > "$STDIN_SEEN"; echo "{\"type\":\"result\",\"is_error\":false}"'
agent_run "TASK BODY HERE" "$repo" "$TEST_TMP/ar.log"
assert_eq "TASK BODY HERE" "$(cat "$STDIN_SEEN")" "the prompt reaches the agent on stdin"

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

# A tool_result carrying is_error:true is an ordinary event: one shell command
# inside the session exited non-zero. Reading it as task failure discards work
# the agent actually finished — the run gets reset --hard and the commit is gone.
cat > "$log" <<'STREAM'
{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"t1"}]}}
{"type":"user","message":{"role":"user","content":[{"type":"tool_result","content":"Exit code 1","is_error":true,"tool_use_id":"t1"}]}}
{"type":"result","subtype":"success","is_error":false,"result":"done"}
STREAM
assert_eq "ok" "$(agent_classify "$log" 0)" "a failed tool call inside a successful run is not a task failure"

# The final result event is what decides, even when earlier events look clean.
cat > "$log" <<'STREAM'
{"type":"user","message":{"role":"user","content":[{"type":"tool_result","is_error":false}]}}
{"type":"result","subtype":"error_during_execution","is_error":true,"result":"boom"}
STREAM
stub_bin claude 'exit 0'
assert_eq "task_failure" "$(agent_classify "$log" 1)" "a failing final result is a task failure"

# A stream with no result event at all and a clean exit is still ok.
printf '{"type":"assistant","message":{}}\n' > "$log"
assert_eq "ok" "$(agent_classify "$log" 0)" "a stream with no result event and exit 0 is ok"

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

# --- intent binding in the prompt ---
# The controller resolved a valid pointer; the path must reach the agent, in the
# task block below the --- TASK --- divider so the prefix stays cachable.
pi=$(agent_prompt 42 "Add a thing" "Body." "autopilot/main" "Demo" "$repo/docs/plans/0001-init.md")
assert_contains "$pi" "0001-init.md" "the intent path reaches the task block"
intent_part=$(printf '%s' "$pi" | sed -n '/^--- TASK/,$p')
assert_contains "$intent_part" "0001-init.md" "the intent path is in the varying task block, not the prefix"
assert_contains "$pi" "read every intent file" "the agent is instructed to read the intent files"

# No intent: no placeholder leaks into the rendered prompt.
assert_eq "0" "$(printf '%s' "$p" | grep -c 'INTENT_FILES' || true)" "no bare placeholder leaks without intent"
prefix_no=$(printf '%s' "$p" | sed -n '1,/^--- TASK/p' | sed '$d')
prefix_yes=$(printf '%s' "$pi" | sed -n '1,/^--- TASK/p' | sed '$d')
assert_eq "$prefix_no" "$prefix_yes" "adding intent paths keeps the prefix byte-identical"
assert_contains "$prefix_yes" "read every intent file" "the read-intent instruction is part of the invariant prefix"

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
