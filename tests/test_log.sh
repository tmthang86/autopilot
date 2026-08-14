#!/bin/sh
. "$(dirname "$0")/harness.sh"
. "$REPO_ROOT/lib/log.sh"

out=$(log_info "hello" 2>&1)
assert_contains "$out" "hello" "log_info emits the message"
assert_contains "$out" "INFO" "log_info is labelled"

out=$(log_error "boom" 2>&1)
assert_contains "$out" "ERROR" "log_error is labelled"

# Logging must never write to stdout — stdout carries return values between libs.
out=$(log_info "quiet" 2>/dev/null)
assert_eq "" "$out" "log_info writes nothing to stdout"

# When a log file is configured, every line must also land there.
AUTOPILOT_LOG_FILE="$TEST_TMP/run.log"
export AUTOPILOT_LOG_FILE
log_warn "persisted" 2>/dev/null
assert_contains "$(cat "$AUTOPILOT_LOG_FILE")" "persisted" "lines are copied to the log file"

finish
