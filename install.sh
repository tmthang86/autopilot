#!/bin/sh
# Install autopilot on a machine and prepare one project for it, in one step.
#
#   curl -fsSL https://raw.githubusercontent.com/tmthang86/autopilot/main/install.sh \
#     | sh -s -- /path/to/project [interval-seconds]
#
# Or download first, read it, then run it:
#   curl -fsSL <same URL> -o install.sh && sh install.sh /path/to/project
#
# This script follows the same shape as the widely used POSIX installers: it
# fetches the runner, prepares the target project, and prints the commands the
# operator runs next. It never starts the loop — starting a program that runs
# with permission checks disabled is the operator's decision, not the installer's
# (ADR-0002), so that step remains explicit.
set -eu

REPO=${AUTOPILOT_REPO:-https://github.com/tmthang86/autopilot.git}
INSTALL_ROOT=${AUTOPILOT_INSTALL_ROOT:-$HOME/.local/share/autopilot}

PROJECT=${1:-}
INTERVAL=${2:-2100}

if [ -z "$PROJECT" ]; then
    printf 'usage: install.sh <project-path> [interval-seconds]\n' >&2
    exit 1
fi

# Dependency check first, each named. A program that runs unsupervised must not
# start half-configured and fail silently later (see observed-behaviour.md).
for dep in git gh jq claude; do
    if ! command -v "$dep" >/dev/null 2>&1; then
        printf 'missing dependency: %s\n' "$dep" >&2
        printf 'Install it, then run this again.\n' >&2
        exit 1
    fi
done

# Prepare the runner. Clone when absent; fast-forward when a checkout is already
# there. --ff-only refuses to touch a checkout that has diverged, so a local
# edit in the deployed copy is never silently overwritten — the operator learns
# about it and resolves it.
if [ ! -d "$INSTALL_ROOT" ]; then
    git clone "$REPO" "$INSTALL_ROOT"
elif [ -d "$INSTALL_ROOT/.git" ]; then
    if ! git -C "$INSTALL_ROOT" pull --ff-only --quiet; then
        printf 'the runner at %s has diverged from its remote; it was left untouched.\n' "$INSTALL_ROOT" >&2
        printf 'Resolve it there, or remove the directory and run this again.\n' >&2
        exit 1
    fi
else
    printf '%s exists but is neither a git checkout nor empty.\n' "$INSTALL_ROOT" >&2
    printf 'Remove it or move it aside, then run this again.\n' >&2
    exit 1
fi

# Prepare the project. This writes <project>/.autopilot/config.json, creates the
# labels the queue queries, and writes the launchd job — but starts nothing.
sh "$INSTALL_ROOT/runner/install-project.sh" "$PROJECT" "$INTERVAL"

cat <<EOF

Review $PROJECT/.autopilot/config.json and commit it before starting —
an uncommitted edit is reverted by the first rejection path.

  sh $INSTALL_ROOT/runner/ctl.sh start  $PROJECT
  sh $INSTALL_ROOT/runner/ctl.sh stop   $PROJECT
  sh $INSTALL_ROOT/runner/ctl.sh status $PROJECT
EOF