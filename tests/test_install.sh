#!/bin/sh
. "$(dirname "$0")/harness.sh"
. "$RUNNER_ROOT/lib/label.sh"

# Never touch the operator's real launchd directories from a test.
PLISTDIR="$TEST_TMP/plists"
AUTOPILOT_PLIST_DIR="$PLISTDIR"
export AUTOPILOT_PLIST_DIR

LCTL_CALLS="$TEST_TMP/launchctl.txt"; export LCTL_CALLS; : > "$LCTL_CALLS"
# `print` answers from LCTL_LOADED, because start and stop each verify their own
# work with it: start must see the job loaded, stop must see it gone.
stub_bin launchctl 'echo "$*" >> "$LCTL_CALLS"
case "$1" in print) [ "${LCTL_LOADED:-1}" = "1" ] || exit 1 ;; esac
exit 0'
LCTL_LOADED=1; export LCTL_LOADED

# Stubbed up front: several installs below (the alpha/beta pair) carry a real
# origin remote, so the installer's label-creation step would otherwise reach
# a real `gh` before the label-specific assertions near the end of this file
# ever run. GH_CALLS is cleared right before each block that inspects it, so
# calls made by earlier installs never leak into a later assertion.
GH_CALLS="$TEST_TMP/gh-calls"; export GH_CALLS
: > "$GH_CALLS"
stub_bin gh 'printf "%s\n" "$*" >> "$GH_CALLS"; exit 0'

repo=$(make_repo)
name=$(basename "$repo")
# An installable project has an origin remote: every gh call the runner makes
# names the repository explicitly, and queue_init refuses to run without one,
# so the installer refuses too. The label is derived from that remote.
git -C "$repo" remote add origin "https://github.com/acme/$name.git"
label=$(label_for_project "$repo")

# --- install ---
sh "$RUNNER_ROOT/install-project.sh" "$repo" 1800 >/dev/null 2>&1
assert_eq "1" "$([ -f "$repo/.autopilot/config.json" ] && echo 1 || echo 0)" "a config is created"
assert_eq "$name" "$(jq -r .project.name "$repo/.autopilot/config.json")" "the project name is substituted"

plist="$PLISTDIR/$label.plist"
assert_eq "1" "$([ -f "$plist" ] && echo 1 || echo 0)" "the launchd plist is written"
assert_contains "$(cat "$plist")" "1800"        "the interval is substituted"
assert_contains "$(cat "$plist")" "$repo"       "the project path is substituted"
assert_contains "$(cat "$plist")" "run-once.sh" "the runner path is substituted"
assert_contains "$(cat "$plist")" "/opt/homebrew/bin" "PATH is set explicitly for launchd"

# No RunAtLoad: the loop must not come back on its own after a reboot.
case "$(cat "$plist")" in
    *"<key>RunAtLoad</key>"*) has=1 ;;
    *)                        has=0 ;;
esac
assert_eq "0" "$has" "the plist carries no RunAtLoad"

# Installing must never start the loop. Enabling a program that runs with
# permission checks disabled is the operator's decision, not the installer's.
assert_eq "" "$(cat "$LCTL_CALLS")" "the installer does not load or start the job"

# The plist must never land in ~/Library/LaunchAgents. launchd auto-loads
# everything there at login, so a job left started before a reboot would come
# back by itself — "off until you start it" would only hold if the operator
# remembered to stop it first.
sh "$RUNNER_ROOT/install-project.sh" "$repo" 1800 >/dev/null 2>&1
assert_eq "0" "$([ -f "$HOME/Library/LaunchAgents/$label.plist" ] && echo 1 || echo 0)" \
    "nothing is written into the auto-loaded LaunchAgents directory"

# Nor may it live inside the project. A project on a volume mounted `noowners`
# — which external drives routinely are — cannot host a plist launchd will
# accept; bootstrap fails with nothing but "Input/output error".
assert_eq "0" "$([ -f "$repo/.autopilot/launchd.plist" ] && echo 1 || echo 0)" \
    "no plist is written inside the project"

# The default is the internal disk, in a directory launchd does not auto-scan.
unset AUTOPILOT_PLIST_DIR
sh "$RUNNER_ROOT/install-project.sh" "$repo" 1800 >/dev/null 2>&1
assert_eq "1" "$([ -f "$HOME/.local/share/autopilot/jobs/$label.plist" ] && echo 1 || echo 0)" \
    "the plist defaults to the internal-disk jobs directory"
rm -f "$HOME/.local/share/autopilot/jobs/$label.plist"
AUTOPILOT_PLIST_DIR="$PLISTDIR"; export AUTOPILOT_PLIST_DIR

# --- gitignore ---
gi=$(cat "$repo/.gitignore")
assert_contains "$gi" ".autopilot/state.json" "state is gitignored"
assert_contains "$gi" ".autopilot/logs/"      "logs are gitignored"
assert_contains "$gi" ".autopilot/STOP"       "the kill switch is gitignored"

# --- idempotence: re-running must not duplicate or clobber ---
jq '.project.name = "EDITED"' "$repo/.autopilot/config.json" > "$TEST_TMP/c" && mv "$TEST_TMP/c" "$repo/.autopilot/config.json"
sh "$RUNNER_ROOT/install-project.sh" "$repo" 1800 >/dev/null 2>&1
assert_eq "EDITED" "$(jq -r .project.name "$repo/.autopilot/config.json")" "an existing config is never overwritten"
assert_eq "1" "$(grep -c '^\.autopilot/STOP$' "$repo/.gitignore")" "gitignore entries are not duplicated"

# --- a non-git directory is an operator error ---
plain="$TEST_TMP/plain"; mkdir -p "$plain"
sh "$RUNNER_ROOT/install-project.sh" "$plain" >/dev/null 2>&1; rc=$?
assert_eq "1" "$rc" "a directory that is not a git repository is refused"

# --- start / stop ---
: > "$LCTL_CALLS"
sh "$RUNNER_ROOT/ctl.sh" start "$repo" >/dev/null 2>&1
calls=$(cat "$LCTL_CALLS")
assert_contains "$calls" "enable"    "start enables the service first"
assert_contains "$calls" "bootstrap" "start bootstraps the job"
# bootstrap refuses a disabled service, so enable must come first.
assert_eq "0" "$(printf '%s' "$calls" | grep -n 'enable' | cut -d: -f1 | head -1 | awk '{print ($1<=1)?0:1}')" "enable precedes bootstrap"

: > "$LCTL_CALLS"
# The job goes away when it is booted out, which is what stop now verifies.
LCTL_LOADED=0
sh "$RUNNER_ROOT/ctl.sh" stop "$repo" >/dev/null 2>&1; rc=$?
LCTL_LOADED=1
assert_eq "0" "$rc" "a stop that removed the job exits 0"
calls=$(cat "$LCTL_CALLS")
assert_contains "$calls" "bootout" "stop boots the job out"
# bootout alone lasts only until the next login; disable is what persists.
assert_contains "$calls" "disable" "stop disables the service so it stays off across reboots"

# Same directory name, different repositories. Before label.sh, the second
# install overwrote the first's plist and both projects shared one job.
one="$TEST_TMP/alpha/svc"; two="$TEST_TMP/beta/svc"
mkdir -p "$one" "$two"
git -C "$one" init -q; git -C "$one" remote add origin https://github.com/alpha/svc.git
git -C "$two" init -q; git -C "$two" remote add origin https://github.com/beta/svc.git

sh "$RUNNER_ROOT/install-project.sh" "$one" 1800 >/dev/null 2>&1
sh "$RUNNER_ROOT/install-project.sh" "$two" 1800 >/dev/null 2>&1

# Filtered to the two "svc" projects specifically: $PLISTDIR already holds
# $repo's own plist from earlier in this file, so an unfiltered count would
# never equal 2 even once alpha and beta each get distinct labels.
assert_eq "2" "$(find "$AUTOPILOT_PLIST_DIR" -name 'com.autopilot.*-svc.plist' | wc -l | tr -d ' ')" \
    "two projects with one directory name get two plists"

# ctl.sh must find the same job the installer wrote, or it manages nothing.
assert_contains "$(sh "$RUNNER_ROOT/ctl.sh" status "$one" 2>&1)" "alpha-svc" \
    "ctl.sh derives the same label the installer used"

# --- a project with no origin remote is refused, not half-installed ---
# queue_init (lib/queue.sh) exits 1 on a project with no origin, on every tick,
# before any guard runs — so not even a STOP file quiets it. The installer used
# to print "no origin remote — skipping label creation", write the plist, and
# exit 0 with the start banner: a green install that could never take a single
# task, which is this codebase's signature failure. It refuses instead.
noorigin=$(make_repo)
: > "$GH_CALLS"
out=$(sh "$RUNNER_ROOT/install-project.sh" "$noorigin" 1800 2>&1); rc=$?
assert_eq "1" "$rc" "a project with no origin remote is refused"
assert_contains "$out" "no origin remote" "the installer says why it refused"
assert_eq "" "$(cat "$GH_CALLS")" "no origin remote — no label creation is attempted"
assert_eq "0" "$([ -f "$PLISTDIR/$(label_for_project "$noorigin").plist" ] && echo 1 || echo 0)" \
    "no launchd job is written for a project that could never run one"
assert_eq "0" "$([ -f "$noorigin/.autopilot/config.json" ] && echo 1 || echo 0)" \
    "nothing is written into a project the installer refused"
case "$out" in *"ctl.sh start"*) banner=1 ;; *) banner=0 ;; esac
assert_eq "0" "$banner" "a refused install does not print the start banner"

fresh="$TEST_TMP/fresh/repo"
mkdir -p "$fresh"
git -C "$fresh" init -q
git -C "$fresh" remote add origin https://github.com/acme/fresh.git

: > "$GH_CALLS"
sh "$RUNNER_ROOT/install-project.sh" "$fresh" 1800 >/dev/null 2>&1

calls=$(cat "$GH_CALLS")
assert_contains "$calls" "label create autopilot"          "the eligibility label is created"
assert_contains "$calls" "label create needs-human"        "the prepare-only label is created"
assert_contains "$calls" "label create blocked"            "the blocked label is created"
assert_contains "$calls" "label create status:in-progress" "the claim label is created"
assert_contains "$calls" "label create model:opus"         "the model labels are created"
assert_contains "$calls" "label create effort:high"        "the effort labels are created"
assert_contains "$calls" "--repo acme/fresh"               "labels are created in the project's own repository"
assert_eq "9" "$(printf '%s\n' "$calls" | grep -c 'label create')" "all nine labels are created"

# A second install must stay clean: an existing label is not a failure worth
# stopping for. Restubbed to fail exactly the way `gh label create` fails on
# a label that already exists — verbatim, observed 2026-08-15 against a real
# repository (see docs/reference/observed-behaviour.md) — so the installer's
# "already exists" branch is chosen because it recognised the real message,
# not because any non-zero exit is assumed to mean that.
stub_bin gh '
name=$3
printf "%s\n" "$*" >> "$GH_CALLS"
printf "label with name \"%s\" already exists; use \`--force\` to update its color and description\n" "$name" >&2
exit 1
'
: > "$GH_CALLS"
sh "$RUNNER_ROOT/install-project.sh" "$fresh" 1800 >/dev/null 2>&1; rc=$?
assert_eq "0" "$rc" "gh reporting a label already exists does not fail the installer"
assert_eq "9" "$(printf '%s\n' "$(cat "$GH_CALLS")" | grep -c 'label create')" \
    "every label is still attempted even though gh reports each one already exists"

# A genuine gh failure — bad credentials, no network, no such repository — is
# not the same as "already exists" and must be reported as what it is, not
# swallowed. install-project.sh has been bitten by this shape of failure
# before (see observed-behaviour.md): something did not work and the thing
# reporting on it said otherwise.
rm -f "$PLISTDIR/$(label_for_project "$fresh").plist"
stub_bin gh '
printf "%s\n" "$*" >> "$GH_CALLS"
printf "HTTP 401: Bad credentials (https://api.github.com/repos/acme/fresh/labels)\n" >&2
exit 1
'
: > "$GH_CALLS"
err=$(sh "$RUNNER_ROOT/install-project.sh" "$fresh" 1800 2>&1 >/dev/null); rc=$?
assert_eq "1" "$rc" "a genuine gh failure fails the installer, not just a printed warning"
assert_contains "$err" "label creation failed" "a real gh failure is named, not mistaken for an existing label"
assert_contains "$err" "HTTP 401: Bad credentials" "the installer surfaces gh's actual error message"
assert_eq "1" "$([ -f "$PLISTDIR/$(label_for_project "$fresh").plist" ] && echo 1 || echo 0)" \
    "the rest of the install (plist included) still completes despite the label failure"

finish
