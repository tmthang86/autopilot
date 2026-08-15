#!/bin/sh
# Start, stop, and inspect a project's autopilot session.
#
# stop uses `launchctl disable` rather than `bootout` alone. bootout lasts only
# until the next login, so a stopped loop would quietly come back after a
# reboot — see docs/decisions/0002-off-until-explicitly-started.md.
set -eu

HERE=$(cd "$(dirname "$0")" && pwd)

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
. "$HERE/lib/label.sh"
LABEL=$(label_for_project "$PROJECT")
# Job files live on the internal disk, keyed by label. A project may sit on a
# volume mounted `noowners` — external drives routinely are — and launchd
# refuses to bootstrap a plist from one, with only "Input/output error" to say
# so. See ADR-0004.
PLIST="${AUTOPILOT_PLIST_DIR:-$HOME/.local/share/autopilot/jobs}/$LABEL.plist"
TARGET="gui/$(id -u)"

case "$ACTION" in
    start)
        if [ ! -f "$PLIST" ]; then
            printf 'no job installed for %s — run install-project.sh first\n' "$NAME" >&2
            exit 1
        fi
        # bootstrap refuses a service that is disabled, so enable comes first.
        launchctl enable "$TARGET/$LABEL" 2>/dev/null || true
        _err=$(launchctl bootstrap "$TARGET" "$PLIST" 2>&1) || true
        # Never report a start that did not happen. launchd failing here is
        # silent otherwise, and the operator walks away believing work is being
        # done all night.
        if ! launchctl print "$TARGET/$LABEL" >/dev/null 2>&1; then
            printf 'FAILED to start autopilot for %s\n' "$NAME" >&2
            [ -n "$_err" ] && printf '  launchctl: %s\n' "$_err" >&2
            printf '  job file: %s\n' "$PLIST" >&2
            exit 1
        fi
        printf 'autopilot started for %s\n' "$NAME"
        printf 'it will take one task every %s seconds while this session lasts\n' \
            "$(sed -n 's/.*<key>StartInterval<\/key><integer>\([0-9]*\)<\/integer>.*/\1/p' "$PLIST" | head -1)"
        ;;
    stop)
        if launchctl print "$TARGET/$LABEL" >/dev/null 2>&1; then _was_loaded=1; else _was_loaded=0; fi
        _err=$(launchctl bootout "$TARGET/$LABEL" 2>&1) || true
        launchctl disable "$TARGET/$LABEL" 2>/dev/null || true
        # Never report a stop that did not happen — the mirror of start above.
        # This branch used to print the reassuring sentence unconditionally, so
        # a job that survived bootout kept taking a task every interval while
        # the operator believed it was off.
        if launchctl print "$TARGET/$LABEL" >/dev/null 2>&1; then
            printf 'FAILED to stop autopilot for %s — the job is still loaded\n' "$NAME" >&2
            [ -n "$_err" ] && printf '  launchctl: %s\n' "$_err" >&2
            printf '  label: %s\n' "$LABEL" >&2
            exit 1
        fi
        printf 'autopilot stopped for %s, and will stay stopped across reboots\n' "$NAME"
        # A stop with nothing to stop is fine, and is also exactly what an
        # upgraded project looks like when its running job was installed under
        # the old com.autopilot.<basename> label: this stops the new label,
        # which nothing is using, while the old job keeps firing.
        if [ "$_was_loaded" -eq 0 ]; then
            printf 'note: no job was loaded under %s. If this project was started before the\n' "$LABEL"
            printf '      job label changed, an older job may still be running — see the\n'
            printf '      upgrade section of docs/guides/install.md.\n'
        fi
        ;;
    status)
        printf 'project:  %s\n' "$PROJECT"
        printf 'job file: %s\n' "$([ -f "$PLIST" ] && printf '%s' "$PLIST" || printf 'not installed')"
        if [ -f "$HERE/VERSION" ]; then
            printf 'runner:   %s\n' "$(cat "$HERE/VERSION")"
        else
            printf 'runner:   unknown — no VERSION file beside ctl.sh\n'
        fi
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
