#!/bin/sh
. "$(dirname "$0")/harness.sh"
. "$RUNNER_ROOT/lib/log.sh"
. "$RUNNER_ROOT/lib/config.sh"
. "$RUNNER_ROOT/lib/intent.sh"

repo=$(make_repo)
mkdir -p "$repo/.autopilot" "$repo/docs/plans"
cp "$RUNNER_ROOT/templates/config.json" "$repo/.autopilot/config.json"
cfg_load "$repo/.autopilot/config.json"

echo "the plan" > "$repo/docs/plans/0001-init.md"
printf 'x\n' > "$repo/api-notes.md"

# The issue body is untrusted text; only the exact marker line at the start of
# a line counts, and every path must resolve inside the project root.
AUTOPILOT_LOG_FILE="$TEST_TMP/intent-happy.log"
export AUTOPILOT_LOG_FILE
: > "$AUTOPILOT_LOG_FILE"

out=$(queue_intent "Intent: docs/plans/0001-init.md" "$repo"); rc=$?
assert_eq "0" "$rc" "a valid intent pointer is accepted"
assert_contains "$out" "docs/plans/0001-init.md" "the resolved path is named"

# The marker is the configured phrase; a bare "#7" or a different marker is
# not intent. The marker line may appear anywhere in the body.
out2=$(queue_intent "Some context.

Intent: api-notes.md

Rest of body." "$repo"); rc=$?
assert_eq "0" "$rc" "the marker is found wherever it appears in the body"
assert_contains "$out2" "api-notes.md" "the second pointer resolved"

# One line, several paths: all are named, and the first line wins.
out3=$(queue_intent "Intent: api-notes.md docs/plans/0001-init.md" "$repo"); rc=$?
assert_eq "0" "$rc" "multiple pointers on one line are accepted"
assert_contains "$out3" "api-notes.md"            "first named path is output"
assert_contains "$out3" "docs/plans/0001-init.md" "second named path is output"

# A custom marker from the config is honoured.
jq '.queue.intent_marker = "Source:"' "$repo/.autopilot/config.json" > "$repo/.autopilot/config.json.tmp"
mv "$repo/.autopilot/config.json.tmp" "$repo/.autopilot/config.json"
cfg_load "$repo/.autopilot/config.json"
out4=$(queue_intent "Source: api-notes.md" "$repo"); rc=$?
assert_eq "0" "$rc" "a custom intent marker from config is honoured"
assert_contains "$out4" "api-notes.md" "path accepted under the custom marker"
jq '.queue.intent_marker = "Intent:"' "$repo/.autopilot/config.json" > "$repo/.autopilot/config.json.tmp"
mv "$repo/.autopilot/config.json.tmp" "$repo/.autopilot/config.json"
cfg_load "$repo/.autopilot/config.json"

# --- refusal cases, each naming the specific reason ---

# No marker line at all.
AUTOPILOT_LOG_FILE="$TEST_TMP/intent-missing.log"
export AUTOPILOT_LOG_FILE
: > "$AUTOPILOT_LOG_FILE"
queue_intent "Just a body with no intent" "$repo"; rc=$?
assert_eq "1" "$rc" "a task naming no intent is refused"
assert_contains "$(cat "$AUTOPILOT_LOG_FILE")" "names no intent" "missing-pointer refusal names the reason"

# A marker line that lists no paths.
AUTOPILOT_LOG_FILE="$TEST_TMP/intent-empty.log"
export AUTOPILOT_LOG_FILE
: > "$AUTOPILOT_LOG_FILE"
queue_intent "Intent:" "$repo"; rc=$?
assert_eq "1" "$rc" "a marker listing no paths is refused"
assert_contains "$(cat "$AUTOPILOT_LOG_FILE")" "lists no paths" "empty-pointer refusal names the reason"

# An absolute path is untrusted input pointing at the address space's own
# layout; no absolute path is ever allowed, regardless of where it points.
AUTOPILOT_LOG_FILE="$TEST_TMP/intent-absolute.log"
export AUTOPILOT_LOG_FILE
: > "$AUTOPILOT_LOG_FILE"
queue_intent "Intent: /etc/passwd" "$repo"; rc=$?
assert_eq "1" "$rc" "an absolute path is refused"
assert_contains "$(cat "$AUTOPILOT_LOG_FILE")" "absolute" "absolute-path refusal names the reason"

# A `..` component can walk out of the tree before resolution. Refuse the
# component itself so the log shows the actual escape rather than a resolved
# path that no longer resembles the input.
AUTOPILOT_LOG_FILE="$TEST_TMP/intent-up.log"
export AUTOPILOT_LOG_FILE
: > "$AUTOPILOT_LOG_FILE"
queue_intent "Intent: ../outside.md" "$repo"; rc=$?
assert_eq "1" "$rc" "a .. component is refused"
assert_contains "$(cat "$AUTOPILOT_LOG_FILE")" "traverses upward" ".. refusal names the reason"

# A path that does not exist may be a typo, but it costs nothing to check and
# refusing early is cheaper than letting the agent improvise around it.
AUTOPILOT_LOG_FILE="$TEST_TMP/intent-missing-file.log"
export AUTOPILOT_LOG_FILE
: > "$AUTOPILOT_LOG_FILE"
queue_intent "Intent: docs/plans/9999-never.md" "$repo"; rc=$?
assert_eq "1" "$rc" "a nonexistent file is refused"
assert_contains "$(cat "$AUTOPILOT_LOG_FILE")" "does not exist" "missing-file refusal names the reason"

# A directory is not a document the agent can be told to read; a pointer must
# name a file.
AUTOPILOT_LOG_FILE="$TEST_TMP/intent-dir.log"
export AUTOPILOT_LOG_FILE
: > "$AUTOPILOT_LOG_FILE"
queue_intent "Intent: docs/plans" "$repo"; rc=$?
assert_eq "1" "$rc" "a directory is refused as intent"
assert_contains "$(cat "$AUTOPILOT_LOG_FILE")" "not a file" "directory refusal names the reason"

# A symlink inside the project pointing at a file outside it defeats a naive
# prefix check. Resolution must canonicalise before comparing.
outside="$TEST_TMP/outside.md"
echo "secret" > "$outside"
ln -s "$outside" "$repo/leak.md"
AUTOPILOT_LOG_FILE="$TEST_TMP/intent-symlink.log"
export AUTOPILOT_LOG_FILE
: > "$AUTOPILOT_LOG_FILE"
queue_intent "Intent: leak.md" "$repo"; rc=$?
assert_eq "1" "$rc" "a symlink escaping the root is refused"
assert_contains "$(cat "$AUTOPILOT_LOG_FILE")" "outside the project root" "symlink-escape refusal names the reason"
rm "$repo/leak.md"

# A symlink resolving inside the root is fine: containment is about where the
# file actually lives, not about whether a link is involved.
ln -s "docs/plans/0001-init.md" "$repo/alias.md"
AUTOPILOT_LOG_FILE="$TEST_TMP/intent-symlink-in.log"
export AUTOPILOT_LOG_FILE
: > "$AUTOPILOT_LOG_FILE"
queue_intent "Intent: alias.md" "$repo"; rc=$?
assert_eq "0" "$rc" "a symlink resolving inside the root is accepted"
rm "$repo/alias.md"

finish