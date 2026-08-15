#!/bin/sh
. "$(dirname "$0")/harness.sh"

# The root install.sh is a wrapper: it fetches the runner, then delegates the
# project preparation to runner/install-project.sh, which is tested on its own.
# What belongs here is the wiring — dependency checks, the clone, and the hard
# rule that the wrapper never starts the loop.
orig_path=$PATH
GH_CALLS="$TEST_TMP/gh-calls"; export GH_CALLS
LCTL_CALLS="$TEST_TMP/lctl-calls"; export LCTL_CALLS

# --- a missing project path is a usage error ---
sh "$REPO_ROOT/install.sh" >/dev/null 2>&1; rc=$?
assert_eq "1" "$rc" "a missing project path is a usage error"

# --- a missing dependency names the tool and stops before any network work ---
# /bin:/usr/bin carries sh, mkdir, cat, and chmod but not the homebrew tools.
# git, gh, and jq are stubbed so the check reaches claude and reports it
# specifically, proving one missing tool is not reported as some earlier one.
stub_bin git 'exit 0'
stub_bin gh  'exit 0'
stub_bin jq  'exit 0'
PATH="$STUB_BIN:/bin:/usr/bin"
export PATH
out=$(sh "$REPO_ROOT/install.sh" "$TEST_TMP/proj" 2>&1); rc=$?
assert_eq "1" "$rc" "a missing dependency fails the install"
assert_contains "$out" "missing dependency: claude" "the exact missing tool is named"
PATH=$orig_path
export PATH

# The happy path clones the real repository into the install root using real
# git, then prepares a real throwaway project. gh stays stubbed so the
# installer's label creation never reaches GitHub from a test.
rm -f "$STUB_BIN/git" "$STUB_BIN/jq"
# claude is only checked for presence here, never invoked in this test; the
# stub keeps the check deterministic regardless of what is on the machine.
stub_bin claude 'exit 0'
: > "$GH_CALLS"
stub_bin gh 'printf "%s\n" "$*" >> "$GH_CALLS"; exit 0'
: > "$LCTL_CALLS"
stub_bin launchctl 'echo "$*" >> "$LCTL_CALLS"; exit 0'

install_root="$TEST_TMP/install-root"
proj=$(make_repo)
git -C "$proj" remote add origin https://github.com/acme/target.git

out=$(AUTOPILOT_INSTALL_ROOT="$install_root" AUTOPILOT_REPO="$REPO_ROOT" \
    AUTOPILOT_PLIST_DIR="$TEST_TMP/plists" \
    sh "$REPO_ROOT/install.sh" "$proj" 1800 2>&1); rc=$?
assert_eq "0" "$rc" "a fresh install succeeds"
assert_eq "1" "$([ -f "$install_root/runner/install-project.sh" ] && echo 1 || echo 0)" \
    "the runner is cloned into the install root"
assert_eq "1" "$([ -f "$proj/.autopilot/config.json" ] && echo 1 || echo 0)" \
    "the project is prepared"
assert_contains "$out" "ctl.sh start" "the closing instructions name the start command"

# The wrapper must never start the loop. Starting a program that runs with
# permission checks disabled is the operator's decision, not the installer's.
assert_eq "" "$(cat "$LCTL_CALLS")" "the wrapper never loads or starts the job"

# --- a checkout that has diverged is left untouched, not force-updated ---
# --ff-only refuses a non-fast-forward, and install.sh must surface that rather
# than silently reset the operator's local edits.
diverged="$TEST_TMP/diverged"
mkdir -p "$diverged/.git"
out=$(AUTOPILOT_INSTALL_ROOT="$diverged" AUTOPILOT_REPO="$REPO_ROOT" \
    sh "$REPO_ROOT/install.sh" "$proj" 1800 2>&1); rc=$?
# The stub is a directory with .git but no real repository, so git pull fails
# for a structural reason. install.sh must report a divergence-shaped message
# and exit non-zero rather than proceeding.
assert_eq "1" "$rc" "a divergence in the existing checkout stops the install"
assert_contains "$out" "left untouched" "the diverged checkout is reported untouched"

finish