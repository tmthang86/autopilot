#!/bin/sh
# Check that every task in a plan carries the fields the delivery chain
# consumes. A task missing Verify or Intent becomes an issue the runner refuses
# at 3 a.m. (ADR-0005), having produced nothing; catching it here costs nothing.
#
# Deliberately not `set -e`: the job is to collect every problem and report them
# together, not to stop at the first. `set -u` alone is safe here because every
# variable below is initialised before use.
set -u

usage() {
    printf 'usage: validate-plan.sh <plan.md> [repo-root]\n' >&2
    exit 2
}

[ $# -ge 1 ] || usage
PLAN=$1
ROOT=${2:-$(cd "$(dirname "$1")/../.." && pwd)}

if [ ! -f "$PLAN" ]; then
    printf 'no such plan: %s\n' "$PLAN" >&2
    exit 2
fi

PROBLEMS=0
CREATED=" "

# Reports every field absent from the block just collected.
check_task() {
    [ -n "$TASK" ] || return 0
    for field in "Done when:" "Verify:" "Intent:" "Depends on:" "Tier:" "Needs human:"; do
        case "$BUF" in
            *"**$field**"*) ;;
            *)
                printf '%s: missing **%s**\n' "$TASK" "$field"
                PROBLEMS=$((PROBLEMS + 1))
                ;;
        esac
    done
    # An Intent that names a file which is not there is worse than a missing
    # one: it reads as satisfied and the runner refuses it anyway. But "not
    # there yet" is not "not there" — a plan legitimately cites a file an
    # earlier task creates, or one a prerequisite plan creates. Existence is
    # therefore checked against the repository PLUS everything CREATED holds.
    _intent=$(printf '%s\n' "$BUF" | sed -n 's/^.*\*\*Intent:\*\*//p')
    for _p in $_intent; do
        case "$_p" in
            —|-|*[!a-zA-Z0-9._/-]*) continue ;;
        esac
        [ -e "$ROOT/$_p" ] && continue
        case "$CREATED" in
            *" $_p "*) continue ;;
        esac
        printf '%s: Intent names a file that does not exist and no task creates: %s\n' "$TASK" "$_p"
        PROBLEMS=$((PROBLEMS + 1))
    done
}

# Every path any task in this plan, or any plan named as a prerequisite, says it
# will create. Collected before the per-task pass, because a task may cite a file
# a LATER task creates when the dependency runs the other way.
collect_created() {
    for _f in "$PLAN" $(sed -n 's/^> \*\*Prerequisite:\*\* *//p' "$PLAN" | tr ',' ' '); do
        [ -f "$_f" ] || [ -f "$ROOT/$_f" ] || continue
        [ -f "$_f" ] || _f="$ROOT/$_f"
        # shellcheck disable=SC2016 # the backtick fence is a literal in the sed pattern
        CREATED="$CREATED $(sed -n 's/^- Create: *//p;s/^| `\([^`]*\)` |.*/\1/p' "$_f" \
            | tr ',' ' ' | tr -d '`' | tr '\n' ' ') "
    done
}

collect_created

TASK=""
BUF=""
FENCE=0

# Fences nest: a plan that teaches the format shows a fenced example inside a
# fenced test file. Counting depth fixes it, with one rule: a fence carrying an
# info string (```sh, ```markdown) opens, and a bare ``` closes. A bare fence met
# at depth zero opens, so an unlabelled top-level block still behaves.
#
# Redirected, never piped: a pipeline runs this loop in a subshell, and PROBLEMS
# would be discarded on exit.
while IFS= read -r line; do
    case "$line" in
        '```'?*)
            FENCE=$((FENCE + 1))
            BUF="$BUF
$line"
            continue
            ;;
        '```')
            if [ "$FENCE" -gt 0 ]; then FENCE=$((FENCE - 1)); else FENCE=1; fi
            BUF="$BUF
$line"
            continue
            ;;
    esac
    if [ "$FENCE" -gt 0 ]; then
        BUF="$BUF
$line"
        continue
    fi
    case "$line" in
        '### '*)
            check_task
            TASK=${line#'### '}
            BUF=""
            ;;
        *)
            BUF="$BUF
$line"
            ;;
    esac
done < "$PLAN"
check_task

[ "$PROBLEMS" -eq 0 ] || exit 1
exit 0
