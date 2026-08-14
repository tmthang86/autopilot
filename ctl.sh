#!/bin/sh
# Start, stop, and inspect a project's autopilot session.
#
# stop uses `launchctl disable` rather than `bootout` alone. bootout lasts only
# until the next login, so a stopped loop would quietly come back after a
# reboot — see docs/decisions/0002-off-until-explicitly-started.md.
set -eu

ACTION=${1:-}
PROJECT=${2:-}

if [ -z "$ACTION" ] || [ -z "$PROJECT" ]; then
    printf 'usage: ctl.sh start|stop|status <project-path>\n' >&2
    exit 1
fi
if [ ! -d "$PROJECT" ]; then
    printf 'no such directory: %s\n' "$PROJECT" >&2
    exit 1
fi
PROJECT=$(cd "$PROJECT" && pwd)

NAME=$(basename "$PROJECT")
LABEL="com.autopilot.$NAME"
PLIST="${AUTOPILOT_PLIST_DIR:-$PROJECT/.autopilot}/launchd.plist"
TARGET="gui/$(id -u)"

case "$ACTION" in
    start)
        if [ ! -f "$PLIST" ]; then
            printf 'no job installed for %s — run install-project.sh first\n' "$NAME" >&2
            exit 1
        fi
        # bootstrap refuses a service that is disabled, so enable comes first.
        launchctl enable "$TARGET/$LABEL" 2>/dev/null || true
        launchctl bootstrap "$TARGET" "$PLIST" 2>/dev/null || true
        printf 'autopilot started for %s\n' "$NAME"
        printf 'it will take one task every %s seconds while this session lasts\n' \
            "$(sed -n 's/.*<key>StartInterval<\/key><integer>\([0-9]*\)<\/integer>.*/\1/p' "$PLIST" | head -1)"
        ;;
    stop)
        launchctl bootout "$TARGET/$LABEL" 2>/dev/null || true
        launchctl disable "$TARGET/$LABEL" 2>/dev/null || true
        printf 'autopilot stopped for %s, and will stay stopped across reboots\n' "$NAME"
        ;;
    status)
        printf 'project:  %s\n' "$PROJECT"
        printf 'job file: %s\n' "$([ -f "$PLIST" ] && printf '%s' "$PLIST" || printf 'not installed')"
        if launchctl print "$TARGET/$LABEL" >/dev/null 2>&1; then
            printf 'loaded:   yes\n'
        else
            printf 'loaded:   no\n'
        fi
        if [ -e "$PROJECT/.autopilot/STOP" ]; then
            printf 'STOP:     present — the runner will stand down at every tick\n'
        fi
        if [ -f "$PROJECT/.autopilot/state.json" ]; then
            _r=$(jq -r '.resume_after // 0' "$PROJECT/.autopilot/state.json")
            if [ "$_r" -gt "$(date +%s)" ]; then
                printf 'paused:   usage window closed for another %ss\n' "$(( _r - $(date +%s) ))"
            fi
            printf 'today:    %s tasks\n' "$(jq -r '.tasks_today // 0' "$PROJECT/.autopilot/state.json")"
        fi
        ;;
    *)
        printf 'usage: ctl.sh start|stop|status <project-path>\n' >&2
        exit 1
        ;;
esac
