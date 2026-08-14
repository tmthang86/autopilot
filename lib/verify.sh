#!/bin/sh
# The verification gate. Nothing gets committed unless this returns 0.

VERIFY_FAILED_NAME=""
VERIFY_OUTPUT=""

verify_run() {
    _cwd=$1
    VERIFY_FAILED_NAME=""
    VERIFY_OUTPUT=""

    _count=$(jq '.verify | length' "$AUTOPILOT_CFG_FILE" 2>/dev/null || printf '0')
    if [ "$_count" -eq 0 ]; then
        log_error "no verify commands configured — refusing to treat unverified work as done"
        return 1
    fi

    _i=0
    while [ "$_i" -lt "$_count" ]; do
        _name=$(jq -r ".verify[$_i].name" "$AUTOPILOT_CFG_FILE")
        _cmd=$(jq -r ".verify[$_i].cmd" "$AUTOPILOT_CFG_FILE")
        _i=$((_i + 1))

        log_info "verify: $_name"
        if ! _out=$( (cd "$_cwd" && sh -c "$_cmd") 2>&1 ); then
            VERIFY_FAILED_NAME=$_name
            VERIFY_OUTPUT=$_out
            log_error "verify failed at: $_name"
            return 1
        fi
    done
    return 0
}
