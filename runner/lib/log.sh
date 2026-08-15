#!/bin/sh
# Logging. Everything goes to stderr so that stdout stays clean for values.
# AUTOPILOT_LOG_FILE, when set, receives a copy.

_log() {
    _level=$1
    shift
    _line="$(date -u '+%Y-%m-%dT%H:%M:%SZ') $_level $*"
    printf '%s\n' "$_line" >&2
    if [ -n "${AUTOPILOT_LOG_FILE:-}" ]; then
        printf '%s\n' "$_line" >> "$AUTOPILOT_LOG_FILE"
    fi
}

log_info()  { _log INFO  "$@"; }
log_warn()  { _log WARN  "$@"; }
log_error() { _log ERROR "$@"; }
