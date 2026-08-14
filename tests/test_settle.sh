#!/bin/sh
. "$(dirname "$0")/harness.sh"
. "$REPO_ROOT/lib/log.sh"
. "$REPO_ROOT/lib/config.sh"
. "$REPO_ROOT/lib/queue.sh"
. "$REPO_ROOT/lib/settle.sh"

repo=$(make_repo)
mkdir -p "$repo/.autopilot"
cp "$REPO_ROOT/templates/config.json" "$repo/.autopilot/config.json"
cfg_load "$repo/.autopilot/config.json"

GH_CALLS="$TEST_TMP/gh-calls.txt"; export GH_CALLS; : > "$GH_CALLS"
stub_bin gh 'echo "$*" >> "$GH_CALLS"; exit 0'

# A real project has a remote. Settling now refuses to close an issue whose work
# has not reached it, so every success path here needs somewhere to push.
git init -q --bare "$TEST_TMP/remote.git"
git -C "$repo" remote add origin "$TEST_TMP/remote.git"
git -C "$repo" checkout -q -b autopilot/main
echo "work" > "$repo/new.txt"

settle_success "$repo" 42 "Add a thing" 0 2>/dev/null
msg=$(git -C "$repo" log -1 --pretty=%B)
assert_contains "$msg" "Add a thing" "the commit subject is the issue title"
assert_contains "$msg" "Closes #42"  "the commit carries the closing trailer"
assert_eq "" "$(git -C "$repo" status --porcelain -- ':(exclude).autopilot')" "the working tree is clean after settling"
assert_contains "$(cat "$GH_CALLS")" "issue close 42" "the issue is closed on full autonomy"

# The agent usually commits its own work, because it is told to run the checks.
# The runner must still push: an issue closed while the commit sits on one
# machine reads as delivered and is not.
echo "agent work" > "$repo/agent.txt"
git -C "$repo" add -A && git -C "$repo" commit -q -m "committed by the agent"
: > "$GH_CALLS"
settle_success "$repo" 50 "Agent committed" 0 2>/dev/null
assert_eq "" "$(git -C "$repo" status -sb | grep -o 'ahead [0-9]*' || true)" "the agent's own commit is pushed too"
assert_contains "$(cat "$GH_CALLS")" "issue close 50" "the issue closes once the work is really on the remote"

# A push that fails must leave the issue open. Closing it would report work as
# delivered while it exists only on this machine.
git -C "$repo" remote set-url origin "$TEST_TMP/does-not-exist.git"
echo "more" > "$repo/unpushable.txt"
: > "$GH_CALLS"
if settle_success "$repo" 51 "Cannot push" 0 2>/dev/null; then rc=0; else rc=1; fi
assert_eq "1" "$rc" "a failed push is reported as a failure"
case "$(cat "$GH_CALLS")" in *"issue close 51"*) closed=1 ;; *) closed=0 ;; esac
assert_eq "0" "$closed" "an issue is NOT closed when the push failed"
git -C "$repo" remote set-url origin "$TEST_TMP/remote.git"

# prepare_only work is committed but NOT closed. If the runner could close work
# whose correctness lives on a screen, the Definition of Done would become a
# formality.
: > "$GH_CALLS"
echo "more" > "$repo/other.txt"
settle_success "$repo" 43 "UI work" 1 2>/dev/null
calls=$(cat "$GH_CALLS")
assert_contains "$calls" "needs-human" "prepare-only work is labelled for a human"
case "$calls" in *"issue close 43"*) closed=1 ;; *) closed=0 ;; esac
assert_eq "0" "$closed" "prepare-only work is NOT closed by the runner"
assert_contains "$(git -C "$repo" log -1 --pretty=%B)" "Closes #43" "prepare-only work is still committed"

# The agent normally commits its own work before the harness verifies it, so by
# the time verification fails HEAD has already moved. Rewinding to HEAD would
# discard only the uncommitted remainder and leave the rejected commit standing
# — turning the verification gate off in exactly the case it exists for.
AUTOPILOT_START_SHA=$(git -C "$repo" rev-parse HEAD)
export AUTOPILOT_START_SHA
echo "rejected work" > "$repo/rejected.txt"
git -C "$repo" add -A && git -C "$repo" commit -q -m "committed by the agent, then rejected"
: > "$GH_CALLS"
settle_failure "$repo" 52 "verify" "suite went red" 2>/dev/null
assert_eq "$AUTOPILOT_START_SHA" "$(git -C "$repo" rev-parse HEAD)" "a rejected agent commit is rewound, not kept"
assert_eq "0" "$([ -f "$repo/rejected.txt" ] && echo 1 || echo 0)" "the rejected work is gone from the tree"
unset AUTOPILOT_START_SHA

# A failed run must leave nothing behind — no partial commit, no stray files.
echo "junk" > "$repo/junk.txt"
mkdir -p "$repo/junkdir" && echo x > "$repo/junkdir/x"
: > "$GH_CALLS"
before=$(git -C "$repo" rev-parse HEAD)
settle_failure "$repo" 44 "verify" "clippy said no" 2>/dev/null
assert_eq "" "$(git -C "$repo" status --porcelain -- ':(exclude).autopilot')" "failure resets the working tree"
assert_eq "$before" "$(git -C "$repo" rev-parse HEAD)" "failure creates no commit"
assert_contains "$(cat "$GH_CALLS")" "issue comment 44" "the failure is reported on the issue"
assert_contains "$(cat "$GH_CALLS")" "clippy said no" "the failure detail reaches the comment"

# The runner must never delete its own directory. .autopilot/ is untracked in a
# project that has not committed its config, and a plain `git clean -fd` would
# wipe config, state, and logs on the first failed task — after which the
# scheduler fires forever into a project that has no configuration.
mkdir -p "$repo/.autopilot/logs"
echo '{"marker":1}' > "$repo/.autopilot/config.json"
echo '{"tasks_today":3}' > "$repo/.autopilot/state.json"
echo "trash" > "$repo/untracked-junk.txt"
settle_failure "$repo" 99 "verify" "detail" 2>/dev/null
assert_eq "1" "$([ -f "$repo/.autopilot/config.json" ] && echo 1 || echo 0)" "the runner's config survives a failure reset"
assert_eq "1" "$([ -f "$repo/.autopilot/state.json" ] && echo 1 || echo 0)" "the runner's state survives a failure reset"
assert_eq "0" "$([ -f "$repo/untracked-junk.txt" ] && echo 1 || echo 0)" "ordinary untracked junk is still cleaned"

# Blocked work is reset and labelled, never guessed at.
echo "half" > "$repo/half.txt"
: > "$GH_CALLS"
settle_blocked "$repo" 45 "Which database?" 2>/dev/null
assert_eq "" "$(git -C "$repo" status --porcelain -- ':(exclude).autopilot')" "blocking resets the working tree"
calls=$(cat "$GH_CALLS")
assert_contains "$calls" "blocked"          "the blocked label is applied"
assert_contains "$calls" "Which database?"  "the question reaches the issue"

# The main branch must never be committed to, whatever the caller asks for.
git -C "$repo" checkout -q main
echo "danger" > "$repo/danger.txt"
before=$(git -C "$repo" rev-parse HEAD)
if settle_success "$repo" 46 "Should not land" 0 2>/dev/null; then rc=0; else rc=1; fi
assert_eq "1" "$rc" "settling on the main branch is refused"
assert_eq "$before" "$(git -C "$repo" rev-parse HEAD)" "no commit lands on the main branch"

# Nothing to commit is a warning, not a crash — the agent may have blocked
# without touching a file.
git -C "$repo" checkout -q -- . 2>/dev/null; rm -f "$repo/danger.txt"
git -C "$repo" checkout -q autopilot/main
: > "$GH_CALLS"
if settle_success "$repo" 47 "Empty change" 0 2>/dev/null; then rc=0; else rc=1; fi
assert_eq "0" "$rc" "an empty change settles without crashing"

finish
