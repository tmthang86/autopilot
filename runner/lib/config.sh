#!/bin/sh
# Config loading. The config is the only place project-specific knowledge lives.

AUTOPILOT_CFG_FILE=""

# Keys without which the runner cannot safely proceed.
_CFG_REQUIRED="project.main_branch project.work_branch queue.ready_label verify agent.permission_mode"

cfg_load() {
    _file=$1
    # Clear first: a rejected load must never leave the previous config
    # selected, or the runner keeps using settings the operator has replaced.
    AUTOPILOT_CFG_FILE=""

    if [ ! -f "$_file" ]; then
        log_error "config not found: $_file"
        return 1
    fi
    if ! jq -e . "$_file" >/dev/null 2>&1; then
        log_error "config is not valid JSON: $_file"
        return 1
    fi
    for _key in $_CFG_REQUIRED; do
        if ! jq -e ".$_key" "$_file" >/dev/null 2>&1; then
            log_error "config is missing required key: $_key"
            return 1
        fi
    done
    AUTOPILOT_CFG_FILE=$_file
    return 0
}

cfg_get() {
    [ -n "$AUTOPILOT_CFG_FILE" ] || { printf '%s' "$2"; return 0; }
    _val=$(jq -r ".$1 // empty" "$AUTOPILOT_CFG_FILE" 2>/dev/null)
    if [ -z "$_val" ]; then
        printf '%s' "$2"
    else
        printf '%s' "$_val"
    fi
}

cfg_list() {
    [ -n "$AUTOPILOT_CFG_FILE" ] || return 0
    jq -r ".$1[]? // empty" "$AUTOPILOT_CFG_FILE" 2>/dev/null
}
