#!/bin/sh
. "$(dirname "$0")/harness.sh"

V="$REPO_ROOT/skills/autopilot-planning/validate-plan.sh"
WORK="$TEST_TMP/plans"
mkdir -p "$WORK/docs/design"
: > "$WORK/docs/design/real.md"

# A well-formed plan with two steps.
cat > "$WORK/good.md" <<'EOF'
# A plan

## Work breakdown

### Task 1: Do the first thing
- **Done when:** the thing is done
- **Verify:** `sh tests/run.sh` → ALL PASS
- **Intent:** docs/design/real.md
- **Depends on:** —
- **Tier:** standard
- **Needs human:** no

### Task 2: Do the second thing
- **Done when:** the other thing is done
- **Verify:** `sh tests/run.sh` → ALL PASS
- **Intent:** docs/design/real.md
- **Depends on:** Task 1
- **Tier:** deep
- **Needs human:** yes
EOF

# The same plan with Verify and Intent removed from task 2.
cat > "$WORK/missing.md" <<'EOF'
# A plan

## Work breakdown

### Task 1: Do the first thing
- **Done when:** the thing is done
- **Verify:** `sh tests/run.sh` → ALL PASS
- **Intent:** docs/design/real.md
- **Depends on:** —
- **Tier:** standard
- **Needs human:** no

### Task 2: Do the second thing
- **Done when:** the other thing is done
- **Depends on:** Task 1
- **Tier:** deep
- **Needs human:** yes
EOF

# A plan whose Intent names a file that is not there.
cat > "$WORK/ghost.md" <<'EOF'
# A plan

## Work breakdown

### Task 1: Do the first thing
- **Done when:** the thing is done
- **Verify:** `sh tests/run.sh` → ALL PASS
- **Intent:** docs/design/does-not-exist.md
- **Depends on:** —
- **Tier:** standard
- **Needs human:** no
EOF

out=$(sh "$V" "$WORK/good.md" "$WORK" 2>&1); rc=$?
assert_eq "0" "$rc"  "a well-formed plan passes"
assert_eq "" "$out"  "a well-formed plan says nothing"

out=$(sh "$V" "$WORK/missing.md" "$WORK" 2>&1); rc=$?
assert_eq "1" "$rc" "a plan with a missing field fails"
assert_contains "$out" "Task 2"      "the failing task is named"
assert_contains "$out" "Verify"      "the missing Verify field is named"
assert_contains "$out" "Intent"      "the missing Intent field is named"
case "$out" in
    *"Task 1"*) reported=yes ;;
    *)          reported=no ;;
esac
assert_eq "no" "$reported" "a well-formed task is not reported"

out=$(sh "$V" "$WORK/ghost.md" "$WORK" 2>&1); rc=$?
assert_eq "1" "$rc" "an Intent naming a missing file fails"
assert_contains "$out" "does-not-exist.md" "the missing intent file is named"

# The label present, nothing after it — a plausible slip copying the
# template under time pressure. The runner refuses this exact task at claim
# time (ADR-0005); the validator must not call it clean.
cat > "$WORK/empty-intent.md" <<'EOF'
# A plan

## Work breakdown

### Task 1: Do the thing
- **Done when:** the thing is done
- **Verify:** `sh tests/run.sh` → ALL PASS
- **Intent:**
- **Depends on:** —
- **Tier:** standard
- **Needs human:** no
EOF

out=$(sh "$V" "$WORK/empty-intent.md" "$WORK" 2>&1); rc=$?
assert_eq "1" "$rc" "an Intent field with no path after it fails"
assert_contains "$out" "names no file" "the empty Intent is named as the problem"

# A heading inside a fence is not a task.
cat > "$WORK/fenced.md" <<'EOF'
# A plan that shows the template

### Task 1: Do the thing
- **Done when:** the thing is done
- **Verify:** `sh tests/run.sh` → ALL PASS
- **Intent:** docs/design/real.md
- **Depends on:** —
- **Tier:** standard
- **Needs human:** no

Here is the shape a task takes:

```markdown
### Task N: <goal, imperative>
- **Done when:** <condition>
```
EOF

out=$(sh "$V" "$WORK/fenced.md" "$WORK" 2>&1); rc=$?
assert_eq "0" "$rc" "a heading inside a fence is not a task"
assert_eq "" "$out" "a fenced example produces no output"

# A path a later task creates is not a typo.
cat > "$WORK/deferred.md" <<'EOF'
# A plan citing what it will create

### Task 1: Cite a file this plan creates later
- **Done when:** it exists
- **Verify:** `sh tests/run.sh` → ALL PASS
- **Intent:** docs/design/not-yet.md
- **Depends on:** —
- **Tier:** standard
- **Needs human:** no

### Task 2: Create it
- **Done when:** written
- **Verify:** `sh tests/run.sh` → ALL PASS
- **Intent:** docs/design/real.md
- **Depends on:** Task 1
- **Tier:** standard
- **Needs human:** no

**Files:**
- Create: docs/design/not-yet.md
EOF

out=$(sh "$V" "$WORK/deferred.md" "$WORK" 2>&1); rc=$?
assert_eq "0" "$rc" "an Intent naming a file a task creates is accepted"
assert_eq "" "$out" "a deferred path produces no output"

# Finally, the hardest real input available: this plan itself.
out=$(sh "$V" "$REPO_ROOT/docs/plans/2026-08-16-planning-skill.md" "$REPO_ROOT" 2>&1); rc=$?
assert_eq "0" "$rc" "this repository's own plan validates"

finish
