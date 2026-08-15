#!/bin/sh
# Deploy the runner from its source — this repository, or the version-stamped
# plugin cache — to the stable path that installed launchd jobs point at.
#
# This runs when a person is present; the unattended loop never calls it. It is
# the only place that writes the deployed copy, which is why it is also the only
# place that must refuse to write over a git checkout on this machine.
set -eu

HERE=$(cd "$(dirname "$0")" && pwd)
DEST=${AUTOPILOT_DEST:-$HOME/.local/share/autopilot}

# A .git directory at the destination means it is a checkout, not a deployment.
# Overwriting it destroys work that may not have been pushed. Refuse and say
# what was found — never "fix" this on its own initiative.
if [ -d "$DEST/.git" ]; then
    printf 'refusing to deploy over %s: it contains a .git directory.\n' "$DEST" >&2
    printf 'That path is a git checkout, not a deployment. Push its work, remove the directory, and deploy again to create a fresh copy.\n' >&2
    exit 1
fi

if [ ! -f "$HERE/VERSION" ]; then
    printf 'no VERSION file beside deploy.sh — this is not a runner source directory: %s\n' "$HERE" >&2
    exit 1
fi

mkdir -p "$DEST"

# The deployment set is exactly what a scheduled run needs, and deploy.sh is
# deliberately not in it: a deployed copy is never edited in place, so it does
# not carry the tool that edits it. jobs/ lives in the deployment root but is
# not part of this set — launchd job files are machine-local, never synchronised.
# Only the directories this script owns are cleared, so a stale file removed
# from the source does not linger in the deployment.
# ${DEST:?} guards the rm: if DEST were ever empty, the expansion aborts
# rather than resolving to /lib or /templates. A deployment that must be
# safe enough to run on an unsupervised machine gets no room for that.
rm -rf "${DEST:?}/lib" "${DEST:?}/templates"
cp "$HERE/run-once.sh" "$DEST/run-once.sh"
cp -R "$HERE/lib" "$DEST/lib"
cp "$HERE/ctl.sh" "$DEST/ctl.sh"
cp "$HERE/install-project.sh" "$DEST/install-project.sh"
cp -R "$HERE/templates" "$DEST/templates"
cp "$HERE/VERSION" "$DEST/VERSION"

printf 'deployed runner %s to %s\n' "$(cat "$DEST/VERSION")" "$DEST"