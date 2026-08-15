#!/bin/sh
# Autopilot: one task per invocation, then exit.
#
# Exit 0 means "ran, or correctly declined to run" — a scheduler must not see a
# normal stand-down as a fault. A non-zero exit means this run could not start
# at all: a broken configuration, or an issue it could not take hold of.
set -eu

AUTOPILOT_HOME=$(cd "$(dirname "$0")" && pwd)
export AUTOPILOT_HOME
PROJECT=""

while [ $# -gt 0 ]; do
    case "$1" in
        --project) PROJECT=${2:-}; shift 2 ;;
        *) printf 'usage: run-once.sh --project <path>\n' >&2; exit 1 ;;
    esac
done
[ -n "$PROJECT" ] || { printf 'usage: run-once.sh --project <path>\n' >&2; exit 1; }

. "$AUTOPILOT_HOME/lib/log.sh"
. "$AUTOPILOT_HOME/lib/config.sh"
. "$AUTOPILOT_HOME/lib/state.sh"
. "$AUTOPILOT_HOME/lib/guards.sh"
. "$AUTOPILOT_HOME/lib/queue.sh"
. "$AUTOPILOT_HOME/lib/agent.sh"
. "$AUTOPILOT_HOME/lib/verify.sh"
. "$AUTOPILOT_HOME/lib/settle.sh"

AP="$PROJECT/.autopilot"
mkdir -p "$AP/logs"
AUTOPILOT_LOG_FILE="$AP/logs/$(date -u +%Y-%m-%d).log"
export AUTOPILOT_LOG_FILE

cfg_load "$AP/config.json" || exit 1
state_init "$AP/state.json"

queue_init "$PROJECT" || exit 1

guard_all "$PROJECT" || exit 0
# Released on every exit path, including the ones that have not been written
# yet. A lock left held wedges the loop until a person notices.
trap 'guard_unlock "$AP"' EXIT

queue_load
ISSUE=$(queue_pick)
if [ -z "$ISSUE" ]; then
    log_info "queue is empty"
    exit 0
fi

TITLE=$(queue_field "$ISSUE" title)
BODY=$(queue_field "$ISSUE" body)
log_info "starting #$ISSUE: $TITLE"

AGENT_MODEL=$(cfg_get agent.default_model sonnet)
AGENT_EFFORT=$(cfg_get agent.default_effort low)
for L in $(printf '%s' "$QUEUE_CACHE" | jq -r ".[] | select(.number == $ISSUE) | .labels[]?.name"); do
    case "$L" in
        model:*)  AGENT_MODEL=${L#model:} ;;
        effort:*) AGENT_EFFORT=${L#effort:} ;;
    esac
done
export AGENT_MODEL AGENT_EFFORT
log_info "#$ISSUE will run on $AGENT_MODEL at $AGENT_EFFORT effort"

WORK_BRANCH=$(cfg_get project.work_branch autopilot/main)
git -C "$PROJECT" checkout -q "$WORK_BRANCH" 2>/dev/null ||
    git -C "$PROJECT" checkout -q -b "$WORK_BRANCH"

PROMPT=$(agent_prompt "$ISSUE" "$TITLE" "$BODY" "$WORK_BRANCH" "$(cfg_get project.name project)") || exit 1

# Where the run began. Every rejection path rewinds to exactly this, because
# the agent commits its own work and HEAD will have moved by then.
AUTOPILOT_START_SHA=$(git -C "$PROJECT" rev-parse HEAD)
export AUTOPILOT_START_SHA

# The claim is what keeps the next wake off this issue, so a claim that did not
# take means this run does not hold it. Spending an agent turn — and a commit —
# on work another run may already be doing is worse than standing down, and the
# operator gets gh's own reason from the log line queue_claim just wrote.
if ! queue_claim "$ISSUE"; then
    log_error "not running the agent for #$ISSUE: the issue was never claimed, and nothing has been changed"
    exit 1
fi
# Guarded, not bare: the counter is bookkeeping, and losing it must not abandon
# an issue this run has already claimed. The write failure is logged by
# state.sh; this says what it costs.
state_bump tasks_today >/dev/null ||
    log_warn "could not record #$ISSUE in tasks_today — the daily cap will not count this run"

RUN_LOG="$AP/logs/run-$ISSUE-$(date -u +%H%M%S).jsonl"
if agent_run "$PROMPT" "$PROJECT" "$RUN_LOG"; then RC=0; else RC=1; fi

case "$(agent_classify "$RUN_LOG" "$RC")" in
    usage_limit)
        BACKOFF=$(( 900 * ( $(state_get backoff_step 0) + 1 ) ))
        if [ "$BACKOFF" -gt 3600 ]; then BACKOFF=3600; fi
        state_set_num resume_after "$(( $(date +%s) + BACKOFF ))"
        state_bump backoff_step >/dev/null
        queue_release "$ISSUE"
        _settle_reset "$PROJECT"
        log_info "usage window closed; retrying in ${BACKOFF}s, #$ISSUE untouched"
        exit 0
        ;;
    task_failure)
        state_bump consecutive_failures >/dev/null
        settle_failure "$PROJECT" "$ISSUE" "agent" "$(tail -c 2000 "$RUN_LOG" 2>/dev/null || true)"
        ;;
    ok)
        state_set_num backoff_step 0
        if verify_run "$PROJECT"; then
            state_set_num consecutive_failures 0
            if queue_has_label "$ISSUE" "$(cfg_get autonomy.prepare_only_label needs-human)"; then
                PREPARE=1
            else
                PREPARE=0
            fi
            # Guarded: settle_success returns non-zero when the push never
            # reached the remote or the branch check refused. Left bare, that
            # ended the run here — before the circuit breaker below could see
            # it, so an unreachable remote could fail every night without ever
            # tripping anything.
            settle_success "$PROJECT" "$ISSUE" "$TITLE" "$PREPARE" ||
                state_bump consecutive_failures >/dev/null ||
                log_warn "could not record the settlement failure for #$ISSUE"
        else
            state_bump consecutive_failures >/dev/null
            settle_failure "$PROJECT" "$ISSUE" "$VERIFY_FAILED_NAME" "$VERIFY_OUTPUT"
        fi
        ;;
esac

if [ "$(state_get consecutive_failures 0)" -ge "$(cfg_get pacing.circuit_breaker_failures 3)" ]; then
    touch "$AP/STOP"
    log_error "circuit breaker tripped after $(state_get consecutive_failures 0) failures — STOP file created"
fi

exit 0
