#!/bin/sh
# Run state. This file is the runner's only memory between invocations,
# so every write must be readable back into the same shape.

AUTOPILOT_STATE_FILE=""

_STATE_DEFAULTS='{
  "resume_after": 0,
  "backoff_step": 0,
  "consecutive_failures": 0,
  "tasks_today": 0,
  "tasks_today_date": "",
  "last_issue": 0
}'

state_init() {
    AUTOPILOT_STATE_FILE=$1
    if [ ! -f "$AUTOPILOT_STATE_FILE" ] || ! jq -e . "$AUTOPILOT_STATE_FILE" >/dev/null 2>&1; then
        [ -f "$AUTOPILOT_STATE_FILE" ] && log_warn "state file unreadable, rebuilding from defaults"
        mkdir -p "$(dirname "$AUTOPILOT_STATE_FILE")"
        printf '%s\n' "$_STATE_DEFAULTS" > "$AUTOPILOT_STATE_FILE"
    fi
    return 0
}

# `// empty` would turn a stored 0 into the caller's default, which is the
# difference between "no backoff" and "unknown". Test for null instead.
state_get() {
    _val=$(jq -r "if .$1 == null then \"\\u0000\" else .$1 end" "$AUTOPILOT_STATE_FILE" 2>/dev/null)
    if [ -z "$_val" ] || [ "$_val" = "$(printf '\000')" ]; then
        printf '%s' "$2"
    else
        printf '%s' "$_val"
    fi
}

# Writes go through a temp file so an interrupted run cannot leave a truncated
# state file behind — that would look identical to corruption on the next wake.
_state_write() {
    _tmp="$AUTOPILOT_STATE_FILE.tmp.$$"
    if jq "$1" "$AUTOPILOT_STATE_FILE" > "$_tmp" 2>/dev/null; then
        mv "$_tmp" "$AUTOPILOT_STATE_FILE"
    else
        rm -f "$_tmp"
        log_error "state write failed: $1"
        return 1
    fi
}

state_set()     { _state_write ".$1 = \"$2\""; }
state_set_num() { _state_write ".$1 = $2"; }

state_bump() {
    _new=$(( $(state_get "$1" 0) + 1 ))
    _state_write ".$1 = $_new" && printf '%s' "$_new"
}
