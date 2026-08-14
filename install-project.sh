#!/bin/sh
# Prepare a project for autopilot. Writes the launchd job but never starts it —
# enabling a program that runs with permission checks disabled is the operator's
# decision. See docs/decisions/0002-off-until-explicitly-started.md.
set -eu

HERE=$(cd "$(dirname "$0")" && pwd)
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
LABEL="com.autopilot.$NAME"
AP="$PROJECT/.autopilot"
AGENTS=${AUTOPILOT_LAUNCHAGENTS_DIR:-$HOME/Library/LaunchAgents}

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

mkdir -p "$AGENTS"
PLIST="$AGENTS/$LABEL.plist"
sed -e "s|{{LABEL}}|$LABEL|g" \
    -e "s|{{RUNNER}}|$HERE/run-once.sh|g" \
    -e "s|{{PROJECT}}|$PROJECT|g" \
    -e "s|{{INTERVAL}}|$INTERVAL|g" \
    -e "s|{{HOME}}|$HOME|g" \
    "$HERE/templates/launchd.plist.tmpl" > "$PLIST"

cat <<EOF

Wrote $PLIST
The job is NOT running. Review $AP/config.json first, then:

  sh $HERE/ctl.sh start $PROJECT

Stop it again with:

  sh $HERE/ctl.sh stop $PROJECT

Pause without stopping the job:

  touch $AP/STOP
EOF
