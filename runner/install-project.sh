#!/bin/sh
# Prepare a project for autopilot. Writes the launchd job but never starts it —
# enabling a program that runs with permission checks disabled is the operator's
# decision. See docs/decisions/0002-off-until-explicitly-started.md.
set -eu

HERE=$(cd "$(dirname "$0")" && pwd)
. "$HERE/lib/label.sh"
PROJECT=${1:-}
INTERVAL=${2:-2100}

if [ -z "$PROJECT" ]; then
    printf 'usage: install-project.sh <project-path> [interval-seconds]\n' >&2
    exit 1
fi
if [ ! -d "$PROJECT" ]; then
    printf 'no such directory: %s\n' "$PROJECT" >&2
    exit 1
fi
PROJECT=$(cd "$PROJECT" && pwd)
if [ ! -d "$PROJECT/.git" ]; then
    printf 'not a git repository: %s\n' "$PROJECT" >&2
    exit 1
fi
# Checked here, before anything is written. Every gh call the runner makes names
# the repository explicitly, derived from origin, so lib/queue.sh's queue_init
# exits 1 without one — on every tick, and before guard_all, so not even a STOP
# file quiets it. Installing anyway produced a green install and a start banner
# for a job that could never take a single task: the same disease as reporting
# a start that did not happen.
if ! SLUG=$(repo_slug_for_project "$PROJECT"); then
    printf 'no origin remote (or none that parses into owner/repo): %s\n' "$PROJECT" >&2
    printf 'The runner names the repository on every gh call and refuses to start without it,\n' >&2
    printf 'so a job installed here would stand down at every wake, forever.\n' >&2
    printf 'Add an origin remote and run this again. Nothing was written.\n' >&2
    exit 1
fi

NAME=$(basename "$PROJECT")
LABEL=$(label_for_project "$PROJECT")
AP="$PROJECT/.autopilot"
# Not ~/Library/LaunchAgents (launchd auto-loads everything there at login) and
# not the project directory (a project on a `noowners` volume cannot host a
# plist launchd will accept). Internal disk, not auto-scanned. See ADR-0004.
PLIST_DIR=${AUTOPILOT_PLIST_DIR:-$HOME/.local/share/autopilot/jobs}

mkdir -p "$AP/logs"

# Never overwrite a config the operator has edited.
if [ -f "$AP/config.json" ]; then
    printf 'keeping existing %s\n' "$AP/config.json"
else
    sed "s|CHANGE_ME|$NAME|" "$HERE/templates/config.json" > "$AP/config.json"
    printf 'created %s — edit its verify commands before starting\n' "$AP/config.json"
fi

# config.json is meant to be committed; everything else here is local.
for entry in ".autopilot/state.json" ".autopilot/logs/" ".autopilot/STOP" ".autopilot/lock/"; do
    if ! grep -qxF "$entry" "$PROJECT/.gitignore" 2>/dev/null; then
        printf '%s\n' "$entry" >> "$PROJECT/.gitignore"
    fi
done

# The runner queries these by name and cannot tell a missing label from an
# empty queue, so a project without them reports no work forever. Creating
# them is idempotent: an existing label is not an error worth stopping for.
# But `gh label create` exits non-zero for that case AND for a bad token, a
# network failure, or a repository that does not exist — collapsing all of
# those into "already exists" would report success over a repository that
# has no labels at all, exactly the kind of quiet lie this repository has
# already been bitten by three times (see docs/reference/observed-behaviour.md,
# 2026-08-15). Only the message `gh` actually prints distinguishes them.
# $SLUG came from repo_slug_for_project (lib/label.sh) above — the one parse of
# the origin remote; lib/queue.sh:12-24 is the other, and a third does not
# belong here.
#
# The ready label's name is not one of the fixed nine: lib/queue.sh reads
# queue.ready_label from config.json at run time, so an installer that
# hardcoded "autopilot" here would agree with the runner only for operators
# who never touch that key. Read the same key from the same file instead —
# $AP/config.json was just written (or kept, if the operator already edited
# it) above — so the label the installer creates and the label the queue
# queries can never drift apart. The other eight names stay fixed; only this
# one is configurable.
READY_LABEL=$(jq -r '.queue.ready_label // empty' "$AP/config.json" 2>/dev/null)
[ -n "$READY_LABEL" ] || READY_LABEL=autopilot

LABEL_FAILURES=0
printf 'creating labels in %s\n' "$SLUG"
_create_label() {
    _lname=$1; _lcolour=$2; _ldesc=$3
    if _lerr=$(gh label create "$_lname" --repo "$SLUG" --color "$_lcolour" \
        --description "$_ldesc" 2>&1 >/dev/null); then
        return 0
    fi
    case "$_lerr" in
        *"already exists"*) printf '  %s already exists\n' "$_lname" ;;
        *)
            LABEL_FAILURES=$((LABEL_FAILURES + 1))
            printf '  %s: label creation failed: %s\n' "$_lname" "$_lerr" >&2
            ;;
    esac
}

_create_label "$READY_LABEL" 0e8a16 "Eligible for unattended execution"
while IFS='|' read -r lname lcolour ldesc; do
    [ -n "$lname" ] || continue
    _create_label "$lname" "$lcolour" "$ldesc"
done <<'LABELS'
needs-human|fbca04|Correctness needs a person to observe it; autopilot must not close
blocked|d93f0b|Waiting on a human decision
status:in-progress|1d76db|Claimed by a run
model:sonnet|c5def5|Run with Sonnet
model:opus|d4c5f9|Run with Opus
effort:low|ededed|Mechanical work
effort:medium|d9d9d9|Moderate reasoning
effort:high|bfbfbf|Hard reasoning
LABELS

mkdir -p "$PLIST_DIR"
PLIST="$PLIST_DIR/$LABEL.plist"
sed -e "s|{{LABEL}}|$LABEL|g" \
    -e "s|{{RUNNER}}|$HERE/run-once.sh|g" \
    -e "s|{{PROJECT}}|$PROJECT|g" \
    -e "s|{{INTERVAL}}|$INTERVAL|g" \
    -e "s|{{HOME}}|$HOME|g" \
    "$HERE/templates/launchd.plist.tmpl" > "$PLIST"

# Checked, and reported, before the success banner below — never after. The
# rest of the install (config, gitignore, plist) is still worth having even
# when a label failed to create, so it all ran above regardless. But the
# queue cannot tell a missing label from an empty one, so a real failure here
# must not be reported as a clean install: an operator reading stdout and
# stderr interleaved must meet the warning before the success-shaped block,
# never the other way around, or the block reads as the whole story. A
# non-zero exit is also the loud signal an operator or a calling script can
# actually check for.
if [ "$LABEL_FAILURES" -gt 0 ]; then
    printf '\nwarning: %d label(s) could not be created — see errors above. Fix and re-run before starting.\n' \
        "$LABEL_FAILURES" >&2
    exit 1
fi

cat <<EOF

Wrote $PLIST (deliberately not in ~/Library/LaunchAgents, which launchd auto-loads)
The job is NOT running. Review $AP/config.json first, then:

  sh $HERE/ctl.sh start $PROJECT

Stop it again with:

  sh $HERE/ctl.sh stop $PROJECT

Pause without stopping the job:

  touch $AP/STOP
EOF
