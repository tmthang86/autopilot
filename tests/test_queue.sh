#!/bin/sh
. "$(dirname "$0")/harness.sh"
. "$REPO_ROOT/lib/log.sh"
. "$REPO_ROOT/lib/config.sh"
. "$REPO_ROOT/lib/queue.sh"

repo=$(make_repo)
mkdir -p "$repo/.autopilot"
cp "$REPO_ROOT/templates/config.json" "$repo/.autopilot/config.json"
cfg_load "$repo/.autopilot/config.json"

# --- the repository must be named explicitly on every gh call ---
# gh resolves the repo from the current working directory. The runner is started
# from wherever the scheduler happens to be, so without an explicit --repo a run
# for one project reads, claims, and closes issues in another.
git -C "$repo" remote add origin "https://github.com/someone/thing.git"
if queue_init "$repo" >/dev/null 2>&1; then rc=0; else rc=1; fi
assert_eq "0" "$rc" "queue_init accepts an https origin"
assert_eq "someone/thing" "$AUTOPILOT_REPO" "the slug is derived from an https remote"

git -C "$repo" remote set-url origin "git@github.com:other/proj.git"
queue_init "$repo" >/dev/null 2>&1
assert_eq "other/proj" "$AUTOPILOT_REPO" "the slug is derived from an ssh remote"

noremote=$(make_repo)
if queue_init "$noremote" >/dev/null 2>&1; then rc=0; else rc=1; fi
assert_eq "1" "$rc" "a project with no origin remote is refused"

git -C "$repo" remote set-url origin "https://github.com/someone/thing.git"
queue_init "$repo" >/dev/null 2>&1

GH_ARGS="$TEST_TMP/gh-args.txt"; export GH_ARGS; : > "$GH_ARGS"
stub_bin gh 'echo "$*" >> "$GH_ARGS"
case "$*" in
  *"issue list"*) echo "[{\"number\":1,\"title\":\"t\",\"body\":\"b\",\"labels\":[{\"name\":\"autopilot\"}],\"milestone\":null}]" ;;
  *"issue view"*) echo "CLOSED" ;;
  *) echo "{}" ;;
esac'
queue_load; queue_pick >/dev/null
queue_claim 1
queue_release 1
missing=$(grep -vc -- "--repo someone/thing" "$GH_ARGS" || true)
assert_eq "0" "$missing" "every gh call names the repository explicitly"

# --- dependency parsing: pure text work, tested on its own ---
assert_eq "7" "$(queue_deps 'Blah blah. Depends on #7. More text.')" "single dependency parsed"
two=$(queue_deps 'Depends on #7
Depends on #12')
assert_contains "$two" "7"  "first of two dependencies parsed"
assert_contains "$two" "12" "second of two dependencies parsed"
assert_eq "" "$(queue_deps 'No dependencies here at all')" "no dependency yields empty"
assert_eq "" "$(queue_deps 'See issue #7 for context')" "a bare issue reference is not a dependency"
assert_eq "7" "$(queue_deps 'depends on #7')" "the phrase is matched case-insensitively"

# --- the recorded fixture must parse ---
# Recorded from the live repository, never hand-written.
fix="$REPO_ROOT/tests/fixtures/gh-issue-list.json"
assert_eq "0" "$(jq -e 'type == "array"' "$fix" >/dev/null 2>&1 && echo 0 || echo 1)" "fixture is a JSON array"
assert_contains "$(jq -r '.[0].labels[].name' "$fix")" "autopilot" "fixture carries real labels"

# --- selection: #10 depends on #11, which is still open ---
stub_bin gh 'case "$*" in
  *"issue list"*) cat <<JSON
[{"number":10,"title":"Blocked one","body":"Depends on #11","labels":[{"name":"autopilot"}],"milestone":null},
 {"number":11,"title":"Ready one","body":"No deps","labels":[{"name":"autopilot"}],"milestone":null}]
JSON
  ;;
  *"issue view"*) echo "OPEN" ;;
  *) echo "{}" ;;
esac'
queue_load; assert_eq "11" "$(queue_pick)" "a blocked issue is skipped in favour of its dependency"

# Once the dependency is closed, the blocked issue becomes eligible.
stub_bin gh 'case "$*" in
  *"issue list"*) cat <<JSON
[{"number":10,"title":"Now ready","body":"Depends on #11","labels":[{"name":"autopilot"}],"milestone":null}]
JSON
  ;;
  *"issue view"*) echo "CLOSED" ;;
  *) echo "{}" ;;
esac'
queue_load; assert_eq "10" "$(queue_pick)" "issue becomes eligible once its dependency closes"

# An unreadable dependency state must leave the dependent blocked. Failing the
# other way lets work run before the thing it depends on exists, and the damage
# reaches a commit before anyone sees it.
stub_bin gh 'case "$*" in
  *"issue list"*) cat <<JSON
[{"number":10,"title":"Blocked one","body":"Depends on #11","labels":[{"name":"autopilot"}],"milestone":null}]
JSON
  ;;
  *"issue view"*) exit 1 ;;
  *) echo "{}" ;;
esac'
queue_load; assert_eq "" "$(queue_pick 2>/dev/null)" "an unreadable dependency state keeps the dependent blocked"

stub_bin gh 'case "$*" in
  *"issue list"*) cat <<JSON
[{"number":10,"title":"Blocked one","body":"Depends on #11","labels":[{"name":"autopilot"}],"milestone":null}]
JSON
  ;;
  *"issue view"*) echo "some unexpected payload" ;;
  *) echo "{}" ;;
esac'
queue_load; assert_eq "" "$(queue_pick 2>/dev/null)" "an unrecognised state payload keeps the dependent blocked"

# --- excluded labels ---
stub_bin gh 'case "$*" in
  *"issue list"*) cat <<JSON
[{"number":30,"title":"Held","body":"x","labels":[{"name":"autopilot"},{"name":"blocked"}],"milestone":null},
 {"number":31,"title":"Free","body":"x","labels":[{"name":"autopilot"}],"milestone":null}]
JSON
  ;;
  *) echo "{}" ;;
esac'
queue_load; assert_eq "31" "$(queue_pick)" "an excluded label removes an issue from the queue"

# --- label reading must use the labels array, never the prose ---
# An issue whose body merely mentions needs-human is not labelled needs-human.
# Reading it as such would leave real work permanently open; the reverse would
# close work nobody reviewed.
stub_bin gh 'case "$*" in
  *"issue list"*) cat <<JSON
[{"number":20,"title":"Real one","body":"This is not needs-human work","labels":[{"name":"autopilot"}],"milestone":null},
 {"number":21,"title":"Human one","body":"plain","labels":[{"name":"autopilot"},{"name":"needs-human"}],"milestone":null}]
JSON
  ;;
  *) echo "{}" ;;
esac'
queue_load; queue_pick >/dev/null
if queue_has_label 21 "needs-human"; then rc=0; else rc=1; fi
assert_eq "0" "$rc" "a genuinely labelled issue is detected"
if queue_has_label 20 "needs-human"; then rc=0; else rc=1; fi
assert_eq "1" "$rc" "a body mentioning the label is NOT treated as labelled"
assert_eq "Real one" "$(queue_field 20 title)" "queue_field reads a cached field"

# --- the cache must survive the caller's subshell ---
# The runner does exactly `ISSUE=$(queue_pick)`. Command substitution runs in a
# subshell, so a cache populated inside queue_pick would vanish on return: the
# title and body would render empty into the agent's prompt, and every label
# lookup would come back false — closing a needs-human issue automatically.
stub_bin gh 'case "$*" in
  *"issue list"*) cat <<JSON
[{"number":77,"title":"Real title","body":"Real body","labels":[{"name":"autopilot"},{"name":"model:opus"}],"milestone":null}]
JSON
  ;;
  *) echo "{}" ;;
esac'
queue_load
picked=$(queue_pick)
assert_eq "77" "$picked" "the issue is picked through command substitution"
assert_eq "Real title" "$(queue_field "$picked" title)" "the title survives the caller's subshell"
assert_eq "Real body"  "$(queue_field "$picked" body)"  "the body survives the caller's subshell"
if queue_has_label "$picked" "model:opus"; then rc=0; else rc=1; fi
assert_eq "0" "$rc" "label lookups still work after picking in a subshell"

# --- empty queue is a normal outcome, not an error ---
stub_bin gh 'case "$*" in *"issue list"*) echo "[]" ;; *) echo "{}" ;; esac'
queue_load; assert_eq "" "$(queue_pick)" "empty queue yields empty, not an error"

# --- gh failing must not look like an empty queue forever, but must not crash ---
stub_bin gh 'exit 1'
queue_load; assert_eq "" "$(queue_pick 2>/dev/null)" "a gh failure degrades to empty rather than crashing"

finish
