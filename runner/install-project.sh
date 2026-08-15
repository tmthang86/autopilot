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
# repo_slug_for_project (lib/label.sh) is the one parse of the origin remote;
# lib/queue.sh:12-24 is the other, and a third copy does not belong here.
LABEL_FAILURES=0
if SLUG=$(repo_slug_for_project "$PROJECT"); then
    printf 'creating labels in %s\n' "$SLUG"
    while IFS='|' read -r lname lcolour ldesc; do
        [ -n "$lname" ] || continue
        if _lerr=$(gh label create "$lname" --repo "$SLUG" --color "$lcolour" \
            --description "$ldesc" 2>&1 >/dev/null); then
            continue
        fi
        case "$_lerr" in
            *"already exists"*) printf '  %s already exists\n' "$lname" ;;
            *)
                LABEL_FAILURES=$((LABEL_FAILURES + 1))
                printf '  %s: label creation failed: %s\n' "$lname" "$_lerr" >&2
                ;;
        esac
    done <<'LABELS'
autopilot|0e8a16|Eligible for unattended execution
needs-human|fbca04|Correctness needs a person to observe it; autopilot must not close
blocked|d93f0b|Waiting on a human decision
status:in-progress|1d76db|Claimed by a run
model:sonnet|c5def5|Run with Sonnet
model:opus|d4c5f9|Run with Opus
effort:low|ededed|Mechanical work
effort:medium|d9d9d9|Moderate reasoning
effort:high|bfbfbf|Hard reasoning
LABELS
else
    printf 'no origin remote — skipping label creation\n'
fi

mkdir -p "$PLIST_DIR"
PLIST="$PLIST_DIR/$LABEL.plist"
sed -e "s|{{LABEL}}|$LABEL|g" \
    -e "s|{{RUNNER}}|$HERE/run-once.sh|g" \
    -e "s|{{PROJECT}}|$PROJECT|g" \
    -e "s|{{INTERVAL}}|$INTERVAL|g" \
    -e "s|{{HOME}}|$HOME|g" \
    "$HERE/templates/launchd.plist.tmpl" > "$PLIST"

cat <<EOF

Wrote $PLIST (deliberately not in ~/Library/LaunchAgents, which launchd auto-loads)
The job is NOT running. Review $AP/config.json first, then:

  sh $HERE/ctl.sh start $PROJECT

Stop it again with:

  sh $HERE/ctl.sh stop $PROJECT

Pause without stopping the job:

  touch $AP/STOP
EOF

# The rest of the install (config, gitignore, plist) is still worth having
# even when a label failed to create, so it all ran above. But the queue
# cannot tell a missing label from an empty one, so a real failure here must
# not be reported as a clean install — a non-zero exit is the loud signal an
# operator or a calling script can actually check for.
if [ "$LABEL_FAILURES" -gt 0 ]; then
    printf '\nwarning: %d label(s) could not be created — see errors above. Fix and re-run before starting.\n' \
        "$LABEL_FAILURES" >&2
    exit 1
fi
