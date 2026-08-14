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

git -C "$repo" checkout -q -b autopilot/main
echo "work" > "$repo/new.txt"

settle_success "$repo" 42 "Add a thing" 0 2>/dev/null
msg=$(git -C "$repo" log -1 --pretty=%B)
assert_contains "$msg" "Add a thing" "the commit subject is the issue title"
assert_contains "$msg" "Closes #42"  "the commit carries the closing trailer"
assert_eq "" "$(git -C "$repo" status --porcelain)" "the working tree is clean after settling"
assert_contains "$(cat "$GH_CALLS")" "issue close 42" "the issue is closed on full autonomy"

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

# A failed run must leave nothing behind — no partial commit, no stray files.
echo "junk" > "$repo/junk.txt"
mkdir -p "$repo/junkdir" && echo x > "$repo/junkdir/x"
: > "$GH_CALLS"
before=$(git -C "$repo" rev-parse HEAD)
settle_failure "$repo" 44 "verify" "clippy said no" 2>/dev/null
assert_eq "" "$(git -C "$repo" status --porcelain)" "failure resets the working tree"
assert_eq "$before" "$(git -C "$repo" rev-parse HEAD)" "failure creates no commit"
assert_contains "$(cat "$GH_CALLS")" "issue comment 44" "the failure is reported on the issue"
assert_contains "$(cat "$GH_CALLS")" "clippy said no" "the failure detail reaches the comment"

# Blocked work is reset and labelled, never guessed at.
echo "half" > "$repo/half.txt"
: > "$GH_CALLS"
settle_blocked "$repo" 45 "Which database?" 2>/dev/null
assert_eq "" "$(git -C "$repo" status --porcelain)" "blocking resets the working tree"
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
