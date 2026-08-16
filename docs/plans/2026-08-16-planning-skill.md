# The planning skill and the documentation set — implementation plan

> **Type:** Plan · **Date:** 2026-08-16 · **Status:** Approved
> **Scope:** Sub-project F of the multi-harness design — `autopilot-planning`, the documentation
> convention it installs, and the mechanical check that makes a plan a contract rather than prose.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give autopilot a third skill that turns an idea into a committed plan the delivery chain
can shard mechanically, and give this repository the documentation convention that skill installs
everywhere else.

**Architecture:** One new skill (Markdown), one POSIX shell validator that checks a plan's steps
carry every field the delivery chain consumes, a set of templates the skill installs into target
projects, and the conversion of this repository's own rules to `AGENTS.md` with `CLAUDE.md` as a
pointer. Nothing under `runner/` is touched.

**Tech Stack:** POSIX shell (`sh`), Markdown, `git`, `gh`, `jq`, `shellcheck`. No new dependency.

**Spec:** [`docs/design/2026-08-16-multi-harness-role-pipeline-design.md`](../design/2026-08-16-multi-harness-role-pipeline-design.md)
— decisions 17–23 and the section *The skill trio, and `autopilot-planning`*.

## Global Constraints

Copied verbatim from the spec and `CLAUDE.md`. Every task's requirements implicitly include these.

- **POSIX shell only** in this plan. The Rust rewrite is a separate plan and must not begin here.
- `shellcheck` clean before every commit.
- Tests run against a **real throwaway git repository** created in a temp directory, never against
  mocks of git. `gh` and the agent CLI are stubbed with fixture scripts on `PATH`.
- **Nothing under `runner/` changes in this plan.** The boundary test in
  `tests/test_run_once.sh:244` must keep passing untouched.
- Conventional Commits. One commit is one coherent change **including its documentation**.
- **`AGENTS.md` is the single source of agent rules** (decision 20). `CLAUDE.md` is a pointer and
  carries no rules of its own.
- A plan step carries a goal in its heading plus six bold fields: `Done when`, `Verify`, `Intent`,
  `Depends on`, `Tier`, `Needs human` (decision 21).
- **In a target project, `autopilot-planning` writes but never commits** (decision 18). This plan
  implements that behaviour; it does not apply it to this repository, where the operator is running
  the plan directly.

---

## Context

The product starts at a queue. `autopilot-deliver` turns an approved plan into issues and
`autopilot-review` reads what came back, but the plan itself has to arrive from somewhere outside
anything this repository provides. `docs/guides/install.md` used to state that as a precondition for
issues; the skill-layer design removed it for issues and left it standing for plans.

That gap costs more than convenience. `autopilot-deliver` phase 3 currently reads a plan as prose and
interprets it into issues. Interpretation is where a step quietly loses its `Intent:` line — and
`queue_intent` refuses such a task at 3 a.m., after the operator has gone to bed, having produced
nothing. The refusal is correct (ADR-0005) and the cost is a wasted night.

This plan closes the front of the chain and makes the plan itself checkable.

## What we already know

Facts established before planning. No guesses in this section.

- **`autopilot-deliver`'s commit gate already exists and is mechanical.** `skills/autopilot-deliver/SKILL.md`
  phase 1 runs `git ls-files --error-unmatch <plan>` and `git status --porcelain <plan>`, and stops
  with no override. Planning therefore needs no new handshake — it must stop where that gate begins.
- **The runner refuses a task with no `Intent:` line before spending a token.** `runner/run-once.sh:62-74`
  calls `queue_intent` before `queue_claim`, so the refusal costs no attempt and does not touch the
  circuit breaker. ADR-0005 records why.
- **A companion project runs this documentation convention today**, and is this runner's first user.
  Measured there on 2026-08-16: `docs/` carries `product/`, `architecture/`, `decisions/` (17 files),
  `reference/`, `guides/`, `explanation/`, and `plans/`; its rules file is 301 lines and its §4 is a
  binding sync table; its `docs/README.md` is 74 lines and holds a status table covering every
  document. That repository is private, so it is described rather than named here.
- **`AGENTS.md` is a cross-tool standard**, governed under the Linux Foundation's Agentic AI
  Foundation and read by Codex, Cursor, and Copilot among others.
- **The test harness API is fixed.** `tests/harness.sh` provides `assert_eq`, `assert_contains`,
  `make_repo`, `stub_bin`, `finish`, and exports `REPO_ROOT`, `RUNNER_ROOT`, `TEST_TMP`. It does
  **not** set `-e`.
- **Two shell traps this repository has already paid for**, both directly relevant to the validator
  built here (`docs/reference/observed-behaviour.md`):
  - A bare `x=$(false)` under `set -eu` aborts the script even inside an `if` body.
  - Command substitution and pipelines run in a subshell, so a counter incremented inside
    `cmd | while read` is lost when the loop ends. The validator must redirect (`done < "$file"`),
    never pipe.
- `tests/run.sh` reports `ALL PASS` across fifteen files as of `697be35`.

## Approach

Nine tasks. Tasks 1–2 make this repository follow the convention it is about to install elsewhere;
tasks 3–4 build the mechanical check and apply it to this repository's own template; tasks 5–6 build
the skill; tasks 7–8 wire it into the delivery chain and the plugin; task 9 exercises the whole thing
against a real repository.

**Files created**

| Path | Responsibility |
|---|---|
| `AGENTS.md` | The engineering rules. The single source |
| `docs/README.md` | The documentation map and the status table for every document |
| `docs/product/open-items.md` | Questions with no answer yet, and accepted technical debt |
| `skills/autopilot-planning/SKILL.md` | The eight-phase skill |
| `skills/autopilot-planning/validate-plan.sh` | The mechanical six-field check |
| `skills/autopilot-planning/templates/*.tmpl` | What the skill installs into a target project |
| `tests/test_agents_md.sh` | The canonical-file convention holds in this repository |
| `tests/test_docs_status.sh` | Every document under `docs/` appears in the status table |
| `tests/test_plan_format.sh` | The validator behaves |

**Files modified**

| Path | Change |
|---|---|
| `CLAUDE.md` | Reduced to a pointer at `AGENTS.md` |
| `docs/plans/_template.md` | Work breakdown becomes the six-field structured form |
| `skills/autopilot-deliver/SKILL.md` | Phase 3 reads the structured breakdown instead of interpreting prose |
| `tests/run.sh` | Runs the three new test files |
| `.claude-plugin/plugin.json`, `.claude-plugin/marketplace.json`, `runner/VERSION` | Version bump |

---

## Work breakdown

### Task 1: `AGENTS.md` becomes canonical, `CLAUDE.md` becomes a pointer

- **Done when:** `AGENTS.md` holds the rules including a binding sync table; `CLAUDE.md` is at most
  twelve lines and names `AGENTS.md`; `tests/test_agents_md.sh` passes.
- **Verify:** `sh tests/test_agents_md.sh` → 5 assertions, `0 failed`
- **Intent:** `docs/design/2026-08-16-multi-harness-role-pipeline-design.md`
- **Depends on:** —
- **Tier:** standard
- **Needs human:** no

**Files:**
- Create: `AGENTS.md` (by `git mv` from `CLAUDE.md`), `tests/test_agents_md.sh`
- Modify: `CLAUDE.md` (replaced with a pointer), `tests/run.sh`

**Interfaces:**
- Produces: `AGENTS.md` as the path every later task and every template refers to for rules.

- [ ] **Step 1: Write the failing test**

Create `tests/test_agents_md.sh`:

```sh
#!/bin/sh
. "$(dirname "$0")/harness.sh"

assert_eq "1" "$([ -f "$REPO_ROOT/AGENTS.md" ] && echo 1 || echo 0)" \
    "AGENTS.md exists"

assert_contains "$(cat "$REPO_ROOT/CLAUDE.md")" "AGENTS.md" \
    "CLAUDE.md names AGENTS.md"

# A pointer that grows rules is a second source of truth, which is the whole
# thing decision 20 exists to prevent.
assert_eq "1" \
    "$([ "$(wc -l < "$REPO_ROOT/CLAUDE.md")" -le 12 ] && echo 1 || echo 0)" \
    "CLAUDE.md is a pointer, not a copy"

# The sync table is what keeps documents true. It must live in the canonical file.
assert_contains "$(cat "$REPO_ROOT/AGENTS.md")" "you must update" \
    "AGENTS.md carries the binding sync table"

# Rule Zero survives the move; losing it in a rename would be silent.
assert_contains "$(cat "$REPO_ROOT/AGENTS.md")" "runs unsupervised" \
    "AGENTS.md still carries Rule Zero"

finish
```

- [ ] **Step 2: Run it and confirm it fails**

```sh
sh tests/test_agents_md.sh
```

Expected: `FAIL AGENTS.md exists`, and `1 failed` or more. `AGENTS.md` does not exist yet.

- [ ] **Step 3: Move the rules**

```sh
git mv CLAUDE.md AGENTS.md
```

- [ ] **Step 4: Add the sync table to `AGENTS.md`**

Insert this as a new §5 in `AGENTS.md`, renumbering the sections after it. The left column lists
things that change in this repository; the right column is what must change with them.

```markdown
## 5. Documentation sync rules

**A stale document is worse than no document**, because it makes the reader confidently wrong.

The table below is binding. Change something on the left, and **you must update the right in the
same commit**. Never defer a documentation update.

| When you change… | You must update |
|---|---|
| A file under `runner/lib/` or `runner/harness/` | The architecture section of the design it implements |
| The config schema in `runner/templates/config.json` | `docs/guides/install.md` and the tier section of the current design |
| An invariant in §2 | The design that states it, plus the test that fails when it is removed |
| Behaviour of `gh`, `git`, or an agent CLI learned the hard way | `docs/reference/observed-behaviour.md` ← **highest priority** |
| A pick of tool, technique, or a reversed decision | A new ADR under `docs/decisions/` |
| Anything an operator does differently | `docs/guides/install.md` |
| A document added, removed, or finished | The status table in `docs/README.md` |
| A question raised with no answer, or debt taken on | `docs/product/open-items.md` |

**"If it cost you, write it down."** Every hour lost to surprising CLI behaviour goes into
`docs/reference/observed-behaviour.md` immediately, with the date and how it was observed. That is
worth more than the next feature.

Before calling anything done, walk this table row by row.
```

- [ ] **Step 5: Replace `CLAUDE.md` with a pointer**

Write exactly this to `CLAUDE.md`:

```markdown
# CLAUDE.md

The engineering rules for this repository live in **[AGENTS.md](AGENTS.md)**, which is the single
source. Read that file.

This file exists only to point there. Nothing else belongs in it: rules copied here would drift
from `AGENTS.md`, and a reader could not tell which copy an agent actually followed.
```

- [ ] **Step 6: Register the test and run the suite**

Add `test_agents_md.sh` to the list in `tests/run.sh`, then:

```sh
sh tests/test_agents_md.sh && sh tests/run.sh
```

Expected: `5 run, 0 failed` for the new file, then `ALL PASS`.

- [ ] **Step 7: Commit**

```sh
git add AGENTS.md CLAUDE.md tests/test_agents_md.sh tests/run.sh
git commit -m "docs: make AGENTS.md canonical and CLAUDE.md a pointer

AGENTS.md is the cross-tool standard other agents read, and copying the same
rules into a file per tool is the stale-document problem the new sync table
exists to prevent. This repository installs that convention elsewhere, so it
follows it here first.

The sync table is new: this repository had the discipline in prose and never
had the binding table that makes it checkable."
```

---

### Task 2: The tracking artifacts — the documentation map and open items

- **Done when:** `docs/README.md` holds a status row for every Markdown file under `docs/`, and
  `docs/product/open-items.md` records the two items already known to be unverified.
- **Verify:** `sh tests/test_docs_status.sh` → every document listed, `0 failed`
- **Intent:** `docs/design/2026-08-16-multi-harness-role-pipeline-design.md`
- **Depends on:** Task 1
- **Tier:** standard
- **Needs human:** no

**Files:**
- Create: `docs/README.md`, `docs/product/open-items.md`, `tests/test_docs_status.sh`
- Modify: `tests/run.sh`

**Interfaces:**
- Consumes: `AGENTS.md` §5 from Task 1, which binds "a document added or finished" to this table.
- Produces: `docs/README.md` — the file the status test and the skill's phase 5 both read.

- [ ] **Step 1: Write the failing test**

Create `tests/test_docs_status.sh`. It walks the real `docs/` tree, so it cannot pass by accident:

```sh
#!/bin/sh
. "$(dirname "$0")/harness.sh"

MAP="$REPO_ROOT/docs/README.md"

assert_eq "1" "$([ -f "$MAP" ] && echo 1 || echo 0)" \
    "the documentation map exists"

assert_eq "1" "$([ -f "$REPO_ROOT/docs/product/open-items.md" ] && echo 1 || echo 0)" \
    "open-items.md exists"

# Every document under docs/ must appear in the status table. A document nobody
# listed is a document nobody maintains.
#
# Redirected, never piped: a pipeline runs the loop in a subshell and the count
# is lost on exit (docs/reference/observed-behaviour.md, 2026-08-14).
missing=""
while IFS= read -r f; do
    rel=${f#"$REPO_ROOT"/docs/}
    case "$rel" in
        README.md|*/_template.md) continue ;;
    esac
    case "$(cat "$MAP")" in
        *"$rel"*) ;;
        *) missing="$missing $rel" ;;
    esac
done <<EOF
$(find "$REPO_ROOT/docs" -name '*.md' | sort)
EOF

assert_eq "" "$missing" "every document under docs/ appears in the status table"

finish
```

- [ ] **Step 2: Run it and confirm it fails**

```sh
sh tests/test_docs_status.sh
```

Expected: `FAIL the documentation map exists`.

- [ ] **Step 3: Write `docs/README.md`**

```markdown
# Autopilot documentation

Docs-as-code: everything here is Markdown, versioned with the code, and updated in the same commit
as the change it describes. The binding sync rules live in [`../AGENTS.md` §5](../AGENTS.md).

Three standards are combined:

- **[arc42](https://arc42.org)** — architecture documentation (`design/`, `decisions/`)
- **[Diátaxis](https://diataxis.fr)** — split by need (`guides/`, `reference/`)
- **[Nygard ADRs](https://adr.github.io)** — decision records (`decisions/`)

## Start here

| If you want to… | Read |
|---|---|
| Know the rules of this project | [../AGENTS.md](../AGENTS.md) |
| **Learn what the CLIs actually do** | [reference/observed-behaviour.md](reference/observed-behaviour.md) — **highest priority** |
| Install and run the loop | [guides/install.md](guides/install.md) |
| Know why something is built this way | [decisions/](decisions/) |

## Status

Documents are written when the work they describe begins, not speculatively. An empty stub is worse
than an honest gap.

| Document | Status |
|---|---|
| [reference/observed-behaviour.md](reference/observed-behaviour.md) | ✅ Current — seven defects and three CLI measurements recorded |
| [guides/install.md](guides/install.md) | ✅ Current |
| [decisions/0001-one-task-per-wake-over-persistent-daemon.md](decisions/0001-one-task-per-wake-over-persistent-daemon.md) | ✅ Accepted |
| [decisions/0002-off-until-explicitly-started.md](decisions/0002-off-until-explicitly-started.md) | ✅ Accepted |
| [decisions/0003-plist-lives-with-the-project.md](decisions/0003-plist-lives-with-the-project.md) | ⚠️ Superseded by ADR-0004 |
| [decisions/0004-job-files-live-on-the-internal-disk.md](decisions/0004-job-files-live-on-the-internal-disk.md) | ✅ Accepted |
| [decisions/0005-intent-binding.md](decisions/0005-intent-binding.md) | ✅ Accepted |
| [design/2026-08-15-skill-layer-design.md](design/2026-08-15-skill-layer-design.md) | ✅ Approved — delivered |
| [design/2026-08-16-multi-harness-role-pipeline-design.md](design/2026-08-16-multi-harness-role-pipeline-design.md) | ✅ Approved — plans being written |
| [plans/2026-08-14-runner-implementation.md](plans/2026-08-14-runner-implementation.md) | ✅ Delivered |
| [plans/2026-08-15-intent-binding.md](plans/2026-08-15-intent-binding.md) | ✅ Delivered — end-to-end run still open |
| [plans/2026-08-15-plugin-foundations.md](plans/2026-08-15-plugin-foundations.md) | ✅ Delivered |
| [plans/2026-08-15-project-bootstrap.md](plans/2026-08-15-project-bootstrap.md) | ⚠️ Superseded by the skill-layer design |
| [plans/2026-08-15-skill-layer.md](plans/2026-08-15-skill-layer.md) | ✅ Delivered |
| [plans/2026-08-16-planning-skill.md](plans/2026-08-16-planning-skill.md) | 📝 Awaiting approval |
| [product/open-items.md](product/open-items.md) | ✅ Current |

## Layout

```
docs/
├── README.md      this map, and the status of every document
├── product/       open questions and accepted debt
├── design/        validated designs, written before the plans that implement them
├── decisions/     ADRs — why it is built this way
├── reference/     what the tools actually do
├── guides/        how to install, configure, tune, troubleshoot
└── plans/         what we are about to do, written before the code
```
```

- [ ] **Step 4: Write `docs/product/open-items.md`**

Seed it with what is already known to be open, taken verbatim from the *Still unverified* section of
the multi-harness design and the *Not yet observed* section of `observed-behaviour.md`:

```markdown
# Open items

Questions with no answer yet, and debt taken on deliberately. An item leaves this file only when it
is answered or paid down — never because it got old.

Bound by [`../AGENTS.md` §5](../AGENTS.md): raise a question with no answer, or take on debt, and it
is recorded here in the same commit.

## Unverified guarantees

| Item | Since | What would settle it |
|---|---|---|
| A reboot: that `stop` survives one, and a job left started does not come back | 2026-08-15 | Reboot the machine and observe `ctl.sh status` before and after |
| Intent binding end to end against the real agent | 2026-08-15 | One issue with a valid `Intent:` line through the throwaway repository, confirming the transcript shows the plan was read before code was written |
| The throttled `rate_limit_event` payload | 2026-08-14 | Observe a real throttle and record the payload verbatim |

## Open questions

| Question | Raised | Why it matters |
|---|---|---|
| Does `opencode` reach `ollama` and `lmstudio` in practice? | 2026-08-16 | It is the only route to a local-model tier: `pi` answers `provider_not_found` for both |
| What shape do `pi --mode json` and `opencode run --format json` produce? | 2026-08-16 | Their adapters cannot ship as proven until measured |

## Accepted debt

| Debt | Taken on | Cost if left |
|---|---|---|
| `ctl.sh status` cannot distinguish a job that loads but never runs from a quiet night | 2026-08-15 | An `EX_CONFIG` failure looks exactly like an empty queue. Scheduled for the Rust rewrite |
| `wake_budget_usd` is best-effort where the harness reports cost | 2026-08-16 | Only `claude` reports cost today; `wake_timeout_s` is the enforceable ceiling |
```

- [ ] **Step 5: Run the test and the suite**

```sh
sh tests/test_docs_status.sh && sh tests/run.sh
```

Expected: `3 run, 0 failed`, then `ALL PASS`. If the third assertion fails it names the documents
missing from the table — add a row for each.

- [ ] **Step 6: Commit**

```sh
git add docs/README.md docs/product/open-items.md tests/test_docs_status.sh tests/run.sh
git commit -m "docs: add the documentation map and open-items register

The repository had fifteen documents and nothing saying which were current,
superseded, or waiting. The status table is an artifact proven in another repository,
and the test makes it self-enforcing: a document nobody listed is a document
nobody maintains.

open-items.md gathers what was previously scattered across the tail sections of
three files, so an unverified guarantee is visible without reading all of them."
```

---

### Task 3: The plan validator

- **Done when:** `validate-plan.sh` exits 0 on a well-formed plan, exits 1 naming every missing field
  on a malformed one, and reports an `Intent:` path that does not exist.
- **Verify:** `sh tests/test_plan_format.sh` → 16 assertions, `0 failed`; `shellcheck -s sh skills/autopilot-planning/validate-plan.sh` → clean
- **Intent:** `docs/design/2026-08-16-multi-harness-role-pipeline-design.md`
- **Depends on:** —
- **Tier:** standard
- **Needs human:** no

**Files:**
- Create: `skills/autopilot-planning/validate-plan.sh`, `tests/test_plan_format.sh`
- Modify: `tests/run.sh`

**Interfaces:**
- Produces: `validate-plan.sh <plan.md> [repo-root]` — exit 0 clean, exit 1 with one line per
  problem on stdout, exit 2 if the plan file does not exist. Task 6's SKILL.md phase 7 invokes it,
  and Task 7's deliver phase 3 invokes it before sharding.

- [ ] **Step 1: Write the failing test**

Create `tests/test_plan_format.sh`:

```sh
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

# A plan that teaches the format shows the template inside a fence. Those
# headings are examples. Reporting them makes the validator useless in exactly
# the plans that explain the format — including this repository's own.
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

# The real case: this repository's own plan embeds a test file that embeds an
# example plan. Toggling in and out reports the inner example as a task.
cat > "$WORK/nested.md" <<'OUTER'
# A plan that shows a test that shows a plan

### Task 1: Do the thing
- **Done when:** the thing is done
- **Verify:** `sh tests/run.sh` → ALL PASS
- **Intent:** docs/design/real.md
- **Depends on:** —
- **Tier:** standard
- **Needs human:** no

```sh
cat > fixture.md <<'EOF'
### Task 99: An example that is not a task
- **Done when:** nothing
EOF
```
OUTER

out=$(sh "$V" "$WORK/nested.md" "$WORK" 2>&1); rc=$?
assert_eq "0" "$rc" "a nested fenced example is not a task"
case "$out" in
    *"Task 99"*) leaked=yes ;;
    *)           leaked=no ;;
esac
assert_eq "no" "$leaked" "the inner example never reaches the report"

# A path a later task creates is not a typo. Measured on this repository's own
# plans: AGENTS.md and tests/conformance.sh both failed a naive existence check
# while being perfectly correct, which would have made every plan unvalidatable.
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
```

- [ ] **Step 2: Run it and confirm it fails**

```sh
sh tests/test_plan_format.sh
```

Expected: every assertion fails; `sh: .../validate-plan.sh: No such file or directory`.

- [ ] **Step 3: Write the validator**

Create `skills/autopilot-planning/validate-plan.sh`:

```sh
#!/bin/sh
# Check that every task in a plan carries the fields the delivery chain
# consumes. A task missing Verify or Intent becomes an issue the runner refuses
# at 3 a.m. (ADR-0005), having produced nothing; catching it here costs nothing.
#
# Deliberately not `set -e`: the job is to collect every problem and report them
# together, not to stop at the first. `set -u` alone is safe here because every
# variable below is initialised before use.
set -u

usage() {
    printf 'usage: validate-plan.sh <plan.md> [repo-root]\n' >&2
    exit 2
}

[ $# -ge 1 ] || usage
PLAN=$1
ROOT=${2:-$(cd "$(dirname "$1")/../.." && pwd)}

if [ ! -f "$PLAN" ]; then
    printf 'no such plan: %s\n' "$PLAN" >&2
    exit 2
fi

PROBLEMS=0
CREATED=" "

# Reports every field absent from the block just collected.
check_task() {
    [ -n "$TASK" ] || return 0
    for field in "Done when:" "Verify:" "Intent:" "Depends on:" "Tier:" "Needs human:"; do
        case "$BUF" in
            *"**$field**"*) ;;
            *)
                printf '%s: missing **%s**\n' "$TASK" "$field"
                PROBLEMS=$((PROBLEMS + 1))
                ;;
        esac
    done
    # An Intent that names a file which is not there is worse than a missing
    # one: it reads as satisfied and the runner refuses it anyway.
    #
    # But "not there yet" is not "not there". A plan legitimately cites a file an
    # earlier task creates, or one a prerequisite plan creates — measured on this
    # repository's own plans, where AGENTS.md and tests/conformance.sh both
    # failed a naive existence check while being perfectly correct. Existence is
    # therefore checked against the repository PLUS everything CREATED contains.
    _intent=$(printf '%s\n' "$BUF" | sed -n 's/^.*\*\*Intent:\*\*//p')
    for _p in $_intent; do
        case "$_p" in
            —|-|*[!a-zA-Z0-9._/-]*) continue ;;
        esac
        [ -e "$ROOT/$_p" ] && continue
        case "$CREATED" in
            *" $_p "*) continue ;;
        esac
        printf '%s: Intent names a file that does not exist and no task creates: %s\n' "$TASK" "$_p"
        PROBLEMS=$((PROBLEMS + 1))
    done
}

# Every path any task in this plan, or any plan named as a prerequisite, says it
# will create. Collected before the per-task pass, because a task may cite a file
# a LATER task creates when the dependency runs the other way.
collect_created() {
    for _f in "$PLAN" $(sed -n 's/^> \*\*Prerequisite:\*\* *//p' "$PLAN" | tr ',' ' '); do
        [ -f "$_f" ] || [ -f "$ROOT/$_f" ] || continue
        [ -f "$_f" ] || _f="$ROOT/$_f"
        # "- Create: `a`, `b`" and "| `path` | ... |" in a Files created table.
        CREATED="$CREATED $(sed -n 's/^- Create: *//p;s/^| `\([^`]*\)` |.*/\1/p' "$_f" \
            | tr ',' ' ' | tr -d '`' | tr '\n' ' ') "
    done
}

collect_created

TASK=""
BUF=""
FENCE=0

# Fences nest: a plan that teaches the format shows a fenced example inside a
# fenced test file. Toggling in and out treats the inner example's headings as
# real tasks — measured on this very plan, which reported ten tasks for nine.
#
# Counting depth fixes it, with one rule: a fence carrying an info string
# (```sh, ```markdown) opens, and a bare ``` closes. A bare fence met at depth
# zero opens, so an unlabelled top-level block still behaves.
#
# Redirected, never piped. A pipeline runs this loop in a subshell, and PROBLEMS
# would be discarded on exit — the trap recorded in observed-behaviour.md on
# 2026-08-14 that emptied the queue cache.
while IFS= read -r line; do
    case "$line" in
        '```'?*)
            FENCE=$((FENCE + 1))
            BUF="$BUF
$line"
            continue
            ;;
        '```')
            if [ "$FENCE" -gt 0 ]; then FENCE=$((FENCE - 1)); else FENCE=1; fi
            BUF="$BUF
$line"
            continue
            ;;
    esac
    if [ "$FENCE" -gt 0 ]; then
        BUF="$BUF
$line"
        continue
    fi
    case "$line" in
        '### '*)
            check_task
            TASK=${line#'### '}
            BUF=""
            ;;
        *)
            BUF="$BUF
$line"
            ;;
    esac
done < "$PLAN"
check_task

[ "$PROBLEMS" -eq 0 ] || exit 1
exit 0
```

- [ ] **Step 4: Make it executable and run the test**

```sh
chmod +x skills/autopilot-planning/validate-plan.sh
sh tests/test_plan_format.sh
```

Expected: `16 run, 0 failed`. The last assertion runs the validator against this plan — the hardest
real input that exists, because it is the plan that embeds the validator's own test.

- [ ] **Step 5: Run shellcheck**

```sh
shellcheck -s sh skills/autopilot-planning/validate-plan.sh
```

Expected: no output. Fix anything it reports before continuing.

- [ ] **Step 6: Register the test and run the suite**

Add `test_plan_format.sh` to `tests/run.sh`, then `sh tests/run.sh`. Expected: `ALL PASS`.

- [ ] **Step 7: Commit**

```sh
git add skills/autopilot-planning/validate-plan.sh tests/test_plan_format.sh tests/run.sh
git commit -m "feat: check a plan's steps mechanically before it is sharded

A step that loses its Intent line becomes an issue the runner refuses at 3 a.m.,
after the operator has gone to bed, having produced nothing. The refusal is
correct; the wasted night is not. This turns that into a check that runs while
the plan is being written.

It also checks that each Intent path exists. An Intent naming a file that is not
there is worse than a missing one: it reads as satisfied and is refused anyway."
```

---

### Task 4: The plan template becomes structured

- **Done when:** `docs/plans/_template.md` shows the six-field task form, and the validator runs
  clean over this very plan.
- **Verify:** `sh skills/autopilot-planning/validate-plan.sh docs/plans/2026-08-16-planning-skill.md .` → exit 0, no output
- **Intent:** `docs/design/2026-08-16-multi-harness-role-pipeline-design.md` `docs/plans/_template.md`
- **Depends on:** Task 3
- **Tier:** standard
- **Needs human:** no

**Files:**
- Modify: `docs/plans/_template.md`

- [ ] **Step 1: Replace the work-breakdown section of `docs/plans/_template.md`**

Replace the table under `## Work breakdown` with:

```markdown
## Work breakdown

Every task carries a goal in its heading and six fields. Each field has exactly one consumer, and
`skills/autopilot-planning/validate-plan.sh` checks that all six are present before the plan is
sharded.

A plan that depends on another declares it in the header, so the validator knows which paths are
merely not created yet:

```
> **Prerequisite:** docs/plans/YYYY-MM-DD-<earlier>.md
```

### Task 1: <goal, imperative>
- **Done when:** <the observable condition that makes this true>
- **Verify:** `<command>` → <what output counts as passing>
- **Intent:** <space-separated committed paths that authorise this task>
- **Depends on:** <Task N, or — >
- **Tier:** <a tier name from the project's ladder>
- **Needs human:** <yes if correctness depends on behaviour a person must observe>

**Files:**
- Create / Modify / Test: exact paths

- [ ] **Step 1: Write the failing test** — the actual test code, not a description
- [ ] **Step 2: Run it and confirm it fails** — the command, and the expected failure
- [ ] **Step 3: Implement** — the actual code
- [ ] **Step 4: Run it and confirm it passes** — the command, and the expected output
- [ ] **Step 5: Commit** — the actual message, saying why

| Field | Consumed by |
|---|---|
| Done when · Verify | the issue body, and the `verify` commands the runner runs |
| **Intent** | `queue_intent`. A task without it is refused before a token is spent (ADR-0005) |
| Depends on | the creation order in `autopilot-deliver` phase 5 |
| Tier | the `tier:<name>` label |
| Needs human | the `needs-human` label, which leaves the issue open for a person |
```

- [ ] **Step 2: Run the validator against this plan**

```sh
sh skills/autopilot-planning/validate-plan.sh docs/plans/2026-08-16-planning-skill.md .
```

Expected: no output, exit 0. If it reports a task, fix that task in this plan — the plan is
required to satisfy its own rule.

- [ ] **Step 3: Commit**

```sh
git add docs/plans/_template.md
git commit -m "docs: make the plan template a contract the chain can read

Deliver phase 3 read a plan as prose and interpreted it into issues.
Interpretation is where a step quietly loses its Intent line. Six named fields
with one consumer each turns sharding into transcription."
```

---

### Task 5: The templates the skill installs

- **Done when:** `skills/autopilot-planning/templates/` holds seven templates, and each contains no
  literal `TODO` or `TBD`.
- **Verify:** `ls skills/autopilot-planning/templates/ | wc -l` → 7; `grep -rl 'TODO\|TBD' skills/autopilot-planning/templates/` → no output
- **Intent:** `docs/design/2026-08-16-multi-harness-role-pipeline-design.md`
- **Depends on:** Task 4
- **Tier:** standard
- **Needs human:** no

**Files:**
- Create: `skills/autopilot-planning/templates/AGENTS.md.tmpl`,
  `docs-README.md.tmpl`, `plan.md.tmpl`, `design.md.tmpl`, `adr.md.tmpl`, `open-items.md.tmpl`,
  `roadmap.md.tmpl`

- [ ] **Step 1: Copy this repository's own artifacts as the starting templates**

The four artifacts written in Tasks 1–4 are the templates, with this project's specifics replaced by
placeholders the skill fills in. Placeholders are written `{{LIKE_THIS}}` and there must be no other
kind.

```sh
mkdir -p skills/autopilot-planning/templates
cp docs/plans/_template.md          skills/autopilot-planning/templates/plan.md.tmpl
cp docs/product/open-items.md       skills/autopilot-planning/templates/open-items.md.tmpl
cp docs/README.md                   skills/autopilot-planning/templates/docs-README.md.tmpl
cp docs/decisions/_template.md      skills/autopilot-planning/templates/adr.md.tmpl
```

- [ ] **Step 2: Strip this project's content out of the copied templates**

In `open-items.md.tmpl` and `docs-README.md.tmpl`, delete every row of every table, leaving the
headings, the prose, and the header row of each table. The prose explains what belongs in each
table; a template that ships this project's rows would seed a stranger's repository with autopilot's
open questions.

Keep in `open-items.md.tmpl` the three section headings — `## Unverified guarantees`,
`## Open questions`, `## Accepted debt` — and their table headers.

- [ ] **Step 3: Write `AGENTS.md.tmpl`**

```markdown
# {{PROJECT_NAME}} — Engineering Rules

{{ONE_PARAGRAPH_DESCRIPTION}}

This project is built like professional software: **plan before code, document everything, and keep
the docs true.**

---

## 1. Rule Zero: plan first, then build

**Never write code without an approved plan.** There is no exception for "this one is small".

```
Request → Plan (docs/plans/) → Operator approves → Build to plan → Update docs → Done
              ↑                                          │
              └────────── plan wrong? fix the plan ──────┘
```

| Situation | Required action |
|---|---|
| New feature, architecture change, large refactor | Write a plan in `docs/plans/`, wait for approval |
| Small fix, copy change, styling | No separate plan, but still update docs if behaviour changes |
| Plan turns out wrong mid-build | **Stop. Fix the plan. Get it re-approved.** Never silently diverge |
| Part of the plan is blocked | Finish everything else in full, then say plainly what was left out and why |

## 2. Read before you touch the code

{{THE_DOCUMENTS_THAT_COST_SOMETHING_TO_LEARN}}

## 3. The documentation set

**Docs-as-code**: everything is Markdown, lives in this repository, and changes in the same commit
as the code it describes. Three standards combined: **arc42**, **Diátaxis**, **Nygard ADRs**.

```
AGENTS.md      the rules — this file, the single source
README.md      what this is, how to run it
docs/
├── README.md      the map, and the status of every document
├── product/       prd · roadmap · glossary · open-items
├── architecture/  how it is built
├── decisions/     ADRs — why it is built that way
├── reference/     what is it — API behaviour, schema, commands
├── guides/        how do I — setup, testing, release
├── explanation/   why — conceptual background
└── plans/         what next — written before the code
```

Keep the Diátaxis modes distinct. Blurring them is how documentation rots.

## 4. Documentation sync rules

**A stale document is worse than no document**, because it makes the reader confidently wrong.

Change something on the left, and **you must update the right in the same commit**.

| When you change… | You must update |
|---|---|
{{SYNC_TABLE_ROWS}}
| A document added, removed, or finished | The status table in `docs/README.md` |
| A question raised with no answer, or debt taken on | `docs/product/open-items.md` |

**"If it cost you, write it down."**

## 5. ADRs

Every **expensive, hard-to-reverse, or contested** decision gets an ADR. Numbered sequentially,
never reused. Status: `Proposed` → `Accepted` → (`Superseded by ADR-NNNN` | `Deprecated`).

**Never edit an accepted ADR's substance.** Repairing a dead link is maintenance; changing the
reasoning is not. The test: *after the edit, does the record still say what it said?*

The most important section is **Consequences**, both good and bad.

## 6. Code standards

{{LANGUAGE_SPECIFIC_RULES}}

## 7. Testing

{{TEST_RULES}}

Never claim something works without running the command and reading its output. If it fails, say so
and show the output.

## 8. Commits

Conventional Commits. One commit is one coherent change **including its documentation**. Commit
bodies say **why**; the diff already says what.

## 9. Definition of Done

- [ ] Built to the approved plan, or the plan was revised and re-approved
- [ ] {{GATES}} clean, tests written and **actually run**, output read
- [ ] The sync table in §4 walked, and every affected document updated
- [ ] An ADR exists if an architectural decision was made
- [ ] Exercised in the real application, not only in tests

If any box is unchecked, report it as **not done** and say which one.

---

Other agents' rule files (`CLAUDE.md`, `.clinerules`, Kiro steering files) are pointers to this
file. Do not copy rules into them.
```

- [ ] **Step 4: Write `design.md.tmpl`**

```markdown
# {{TITLE}} — design

> **Type:** Design · **Date:** {{DATE}} · **Status:** Draft | Approved
> **Scope:** {{WHAT_THIS_COVERS_AND_WHAT_IT_DOES_NOT}}

## Context

What is true today, what is missing, and why that costs something. Cite files and measurements.

## Prior art

What was found by searching before designing, with URLs. Three parts, all required:

- **Adopted** — standards, patterns, or measurements taken from elsewhere, named rather than
  re-described as though original.
- **Refused** — attractive ideas examined and rejected, each with its reason. An idea rejected
  without a recorded reason gets adopted later by someone who does not know it was considered.
- **Corroborating or contradicting** — independent work that reached the same conclusion from a
  different direction, or reached a different one. A contradiction is answered here, not omitted.

## What was proven before this was written

Every uncertainty named in phase 3b, and how it was settled. Link the probe and its recorded output
under `docs/design/poc/`.

| Claim | How it was settled | Result |
|---|---|---|
| | measured `<date>` · cited `<url>` · **unproven** | |

A design may contain an unproven claim. It may never contain an unmarked one.

## Decisions taken

| # | Decision | Consequence |
|---|---|---|

Each row is an operator decision made on a date, not an inference.

## Architecture

The structure, the boundaries between its parts, and what is checkable rather than promised.

## Errors and testing

How each failure mode is designed for rather than discovered.

## Effect on existing documents

Which documents this changes, and how.

## Out of scope

What this deliberately does not cover, so scope creep is visible rather than silent.

## Still unverified

What is argued rather than evidenced. Moves to `docs/product/open-items.md` when the design is
approved.
```

- [ ] **Step 5: Write `roadmap.md.tmpl`**

The milestone register. It is what says when work may be merged to the main branch, so it is not
optional decoration:

```markdown
# {{PROJECT_NAME}} — roadmap

Milestones in order. A milestone is closed — and only then merged to `{{MAIN_BRANCH}}` — when every
one of its exit criteria holds. "It compiles" is not an exit criterion.

## {{M0}} — {{TITLE}}

**Goal:** one sentence.

**Exit criteria**

- [ ] <observable, checkable by someone who did not build it>
- [ ] <…>

**Out of scope for this milestone:** what deliberately waits.

---

Repeat per milestone. Do not write a milestone's detail before the one before it has closed:
detail written early is detail rewritten.
```

- [ ] **Step 6: Verify no placeholder leaked in**

```sh
ls skills/autopilot-planning/templates/ | wc -l
grep -rl 'TODO\|TBD' skills/autopilot-planning/templates/
```

Expected: `7`, then no output from the grep. `{{PLACEHOLDER}}` markers are intentional and are not
matched by that grep.

- [ ] **Step 7: Commit**

```sh
git add skills/autopilot-planning/templates/
git commit -m "feat: add the documents autopilot-planning installs

The templates are this repository's own artifacts with its content stripped
out, not idealised versions of them. A template nobody uses drifts from the
thing it claims to model; these cannot, because they were copied from files
that are under test here."
```

---

### Task 6: The `autopilot-planning` skill

- **Done when:** `skills/autopilot-planning/SKILL.md` exists with valid frontmatter and describes all
  nine phases, both hard gates, the no-idea branch of phase 0, the four sub-steps of phase 3
  including prior-art research and the committed PoC, the fresh-sub-agent reviews at phases 4 and 7b
  with their three brief rules, the three dispositions and the one-re-review bound at 7c and 7d, and
  the rule that it commits nothing.
- **Verify:** `head -4 skills/autopilot-planning/SKILL.md` → shows `name:` and `description:`; `grep -c '^## Phase' skills/autopilot-planning/SKILL.md` → 9
- **Intent:** `docs/design/2026-08-16-multi-harness-role-pipeline-design.md`
- **Depends on:** Task 5
- **Tier:** deep
- **Needs human:** yes

**Files:**
- Create: `skills/autopilot-planning/SKILL.md`

**Interfaces:**
- Consumes: the templates from Task 5, and `validate-plan.sh` from Task 3 at phase 7.
- Produces: a committed-by-the-operator plan in `docs/plans/`, which `autopilot-deliver` phase 1
  then gates on.

`Needs human: yes` because a skill's correctness is whether it produces the right behaviour when
run, and that is behaviour a person has to observe. Task 9 is where it is observed.

- [ ] **Step 1: Write the frontmatter and the flow**

Create `skills/autopilot-planning/SKILL.md`:

```markdown
---
name: autopilot-planning
description: Turn an idea into a committed plan the unattended runner can be given — brainstorm, design, then plan. Use at the start of a project or a milestone, before autopilot-deliver. Also use when there is time but no particular idea: it proposes what is next from the project's own roadmap, open items, and blocked issues rather than asking a blank question.
---

# autopilot-planning

The front of the chain. It takes an idea, argues it into a design, installs the documentation
convention if the project lacks it, and writes a plan whose steps can be sharded into issues without
interpretation.

It is the first of three: **`autopilot-planning` → `autopilot-deliver` → `autopilot-review`.**

## When to use

- A project or a milestone is starting and there is no plan yet.
- A project has a plan but not the documentation set the delivery chain expects.
- **The operator has time and no particular idea.** Phase 0 proposes what is next from the
  project's own registers rather than asking a blank question.

## It writes. It does not commit.

**The operator commits.** `autopilot-deliver` phase 1 checks mechanically that the plan is tracked
and clean, and that check is the only approval a tired operator cannot wave through. A skill that
commits its own plan deletes it.

Say so plainly when stopping: which files were written, and that committing them is the approval.

## Flow

```
0  Identify the work      idea given → use it;  none given → propose from the project itself
   then classify         spike | bounded | architectural
1  Survey                new repository → scaffold;  existing → read what is there
2  Brainstorm            one question at a time, 2–3 approaches with a recommendation
3  Design                3a research prior art -> 3b name uncertainties ->
                        3c prove them with a committed PoC -> 3d only then draft
4  ▮ TWO-LENS REVIEW     fresh sub-agent PER LENS — product, then engineering
5  Documentation set     AGENTS.md + pointers · the docs/ tree · sync table · status table
6  Plan                  docs/plans/YYYY-MM-DD-<topic>.md, structured work breakdown
7  Check                 7a mechanical here -> 7b fresh sub-agent -> 7c triage every
                        finding -> 7d at most ONE re-review, then stop
8  ▮ STOP                what was written · what review found · what is unproven ·
                        the operator reads, approves, and commits
```

## Phase 0 — identify the work, then classify it

**If the operator arrived with an idea, use it.** If they arrived with nothing, do not ask *"what
would you like to build?"* — the project already knows, and asking a blank question wastes the one
thing the operator came here to be spared.

Read the tracking artifacts and **propose candidates**, newest evidence first:

| Source | What it offers |
|---|---|
| `docs/product/roadmap.md` | The next milestone whose predecessor has closed, and its exit criteria |
| `docs/product/open-items.md` | Unverified guarantees, open questions, and accepted debt |
| open issues labelled `blocked` | Questions the runner stopped on and nobody has answered |
| open `needs-human` issues with commits | Work finished but never accepted |
| `docs/README.md` status table | Documents marked ⏳ Pending whose milestone has arrived |

Present them as a short list with what each would cost and why it might be next, and let the
operator choose. Only when every one of those is genuinely empty — a new repository with no history
— ask what they want to build.

This closes a loop rather than adding a step: `autopilot-review` surfaces these items each morning
and `open-items.md` records them, so the register that ends one cycle begins the next.

**Then classify, and say the classification out loud.**

- **Spike** — a feasibility question whose output is an answer, not code. No design, no plan, and
  **nothing reaches the delivery chain**. That is correct: a spike deliberately produces no work for
  the runner. Report the finding and stop.
- **Bounded** — a well-scoped change to a flow that already exists here. **It still produces a plan
  file**, because the plan file is the interface to `autopilot-deliver` and there is no other way in.
  What shrinks is everything around it: fewer questions in phase 2, one lens instead of two in
  phase 4, and the design becomes a paragraph in the plan's *Context* rather than its own document.
  One to three tasks is normal.
- **Architectural** — a new project, a new subsystem, or a change to how components fit together.
  The full flow below.

**Bounded and architectural differ in size, never in kind.** A bounded change that skipped the plan
would have to be implemented by hand, which is a different product than the one this chain exists to
be. If a change is genuinely too small to be worth an issue, say so plainly and let the operator do
it directly — do not produce a plan nobody wanted.

When in doubt, take the heavier path. Hidden complexity upgrades the path mid-task; nothing
downgrades it.

## Phase 1 — survey

Read before proposing. On an existing project: the current `AGENTS.md` or `CLAUDE.md`, `docs/`,
recent commits, and whatever `docs/reference/` records about behaviour learned the hard way. On a
new one: there is nothing to read, and phase 5 will scaffold.

State which case it is.

## Phase 2 — brainstorm

One question per message. Prefer multiple choice. Understand purpose, constraints, and what would
count as success before proposing anything.

Then propose two or three approaches with trade-offs, lead with a recommendation, and say why.
YAGNI ruthlessly.

## Phase 3 — the design, and nothing uncertain enters it unproven

The most expensive mistake available here is a design that reads well and rests on a technical claim
nobody checked. It gets approved, sharded into issues, and discovered at 3 a.m. by a runner that
cannot ask.

So phase 3 has four steps, and drafting is the last of them.

### 3a — find out how this has already been solved

**Do this first, before naming a single uncertainty of your own.** Search the web for the class of
problem, not just for the API you happen to be reaching for. Almost nothing here is new, and the
cost of not looking is not embarrassment — it is spending a week rediscovering a failure mode
somebody documented years ago, and building a worse version of a thing that has a name.

Search for, at minimum:

| Look for | Because |
|---|---|
| Established standards for this kind of artifact | Adopting one beats inventing a fourth, and it gives every later reader a name to look up |
| How comparable systems solve it, and what they measured | Numbers from someone who ran it beat reasoning from someone who did not |
| Known failure modes and post-mortems | The cheapest lesson is the one already paid for |
| Recent changes — changelogs, deprecations, issue trackers | An answer that was right two years ago is a trap now |

**Bring back three things, and put all three in the design:**

1. **What to adopt**, named and cited with a URL — never re-described as though it were original.
2. **What to refuse, and why.** An attractive idea rejected without a recorded reason gets adopted
   later by someone who does not know it was considered. This is as load-bearing as the adoptions.
3. **What corroborates or contradicts the design's own reasoning.** Independent agreement reached
   from a different direction is the strongest evidence available at this stage; a contradiction is
   more valuable still, and must be answered rather than omitted.

This step is not optional and is not a formality. In the design that produced this plan it supplied
the entire documentation convention, the review-lens pattern, an eight-fold cost measurement that
reframed the architecture, and three ideas that were examined and deliberately refused.

### 3b — name the uncertainties out loud

Now list every technical claim the design will rest on that you do not *know* to be true. Being
explicit is the whole point: **an uncertainty nobody named is one nobody proved.**

Typical shapes: does this CLI accept that flag · what does it return when it fails · does this
endpoint exist · does the algorithm handle the nested case · will this survive a restart · is that
number still true.

Search again for each, this time narrowly: the documentation, the changelog, the issue tracker.
Cite what you find with URLs so the next reader can check it.

**Documentation is a lead, never a verdict.** When a claim can be measured, measure it — measuring
beat reading twice while this design was written. `pi auth check` exits 0 even for a provider with
no credentials, so reading the exit status reports it ready; and `opencode` was documented as
working while every command on this machine failed on an invalid config. No manual says either.

### 3c — prove what is left with a PoC

For every uncertainty that survives 3b, write **the smallest script that fails if the assumption is
wrong**, run it, and read the output. Not a demonstration that it works — a test that would catch it
not working.

```
docs/design/poc/YYYY-MM-DD-<topic>/
├── <name>.sh          the probe, re-runnable by anyone
└── RESULT.md          the command, the real output, the date, and what it settled
```

The scripts are **committed, not thrown away**. A proof someone can re-run next year is worth much
more than a sentence claiming it was run once. When a dependency upgrades, the probe is how you find
out what changed.

**Record failures too, and record them first.** A PoC that fails is the more valuable result: it
changes the design while changing it is still free. A PoC that succeeds records *how* — the exact
command and the real output — because "it works" without the method is not reproducible and is
therefore not evidence.

Where the finding is about an external tool's behaviour, it also goes into
`docs/reference/observed-behaviour.md` with its date. That file already carries that obligation and
is the highest-priority document in most projects using this convention.

### 3d — then draft

Write it to `docs/design/YYYY-MM-DD-<topic>-design.md` from `templates/design.md.tmpl`. Present it
in sections scaled to their complexity, and ask after each whether it holds.

Every claim in the finished design is now one of three things, and **which one must be visible**:

| Kind | How it appears |
|---|---|
| Measured | Stated with the date it was measured and, where it matters, the real output |
| Cited | Stated with the URL, and marked as read rather than run |
| Neither | Listed in *Still unverified* — never buried in a paragraph as though settled |

**A design may contain an unproven claim. It may never contain an unmarked one.** The section that
carries them moves to `docs/product/open-items.md` when the design is approved, so nothing quietly
becomes true by being old.

Record every operator decision in the *Decisions taken* table with its date. A decision that is not
written down gets relitigated.

## Phase 4 — the two-lens review, by an outside voice

Before the design is approved it is read by **fresh sub-agents, one per lens** — not re-read by the
session that wrote it.

This is the same argument the design itself makes about the tester: by the time a design is
finished, the writer's context holds every assumption that produced it, so a self-review reads the
design from inside the reasoning it is supposed to question. Re-reading with a different heading
does not fix that. **A different context does.**

**The product lens.** Is this the problem worth solving, as the user meets it? What is the deeper
opportunity the current framing misses? What would make this unnecessary?

**The engineering lens.** Architecture, data flow, state transitions, and the edge cases. Where does
this fail under concurrency, partial failure, or a restart? What is asserted rather than checked?

### How the brief is written, and this part is easy to get wrong

Three rules, and each exists because breaking it silently destroys the independence being paid for:

1. **Name paths; never paste excerpts.** The agent reads the design, the project's `AGENTS.md`, and
   the documents the design cites, itself. A pasted excerpt is stale the moment the file changes,
   and it is the agent's own read that puts the material where its attention can reach it.
2. **Never hand over the conversation.** No transcript, no summary of what was decided and why. That
   transcript is precisely the anchoring the fresh context exists to escape — passing it on buys
   the cost of a sub-agent and none of the benefit.
3. **State the question, never the expected answer.** "Check the design handles a restart" is a
   question. "Confirm the design handles a restart correctly" is an instruction to agree, and it
   will be obeyed.

### What comes back

Each lens reports what it found, **including finding nothing** — a review that always approves is
not a review, and a lens that reports nothing twice in a row is a lens worth replacing.

Findings go **to the operator**, not into the design automatically. The skill may not decide that a
reviewer is wrong, and it may not quietly adopt a finding either: both are decisions, and phase 8
belongs to a person.

## Phase 5 — the documentation set

Only what is missing or has drifted. On an established project this is a check, not a rewrite.

- `AGENTS.md` from `templates/AGENTS.md.tmpl` — **the single source of rules**. Every other agent's
  rule file (`CLAUDE.md`, `.clinerules`, Kiro steering files) becomes a one-line pointer to it.
  Never copy rules into them.
- The `docs/` tree, `docs/README.md` with its status table, and `docs/product/open-items.md`.
- The **sync table** in `AGENTS.md` §4: this project's own bindings, not a generic list. Propose
  rows from what the repository actually contains; the operator confirms.

Rewriting a large existing documentation set is a task for a plan, not a side effect of running a
skill. Report the drift and stop.

## Phase 6 — the plan

From `templates/plan.md.tmpl`, to `docs/plans/YYYY-MM-DD-<topic>.md`.

Every task carries a goal in its heading and six fields:

| Field | Consumed by |
|---|---|
| Done when · Verify | the issue body, and the `verify` commands the runner runs |
| **Intent** | `queue_intent`. A task without it is refused before a token is spent |
| Depends on | the creation order in deliver phase 5 |
| Tier | the `tier:<name>` label |
| Needs human | the `needs-human` label, leaving the issue open for a person |

Steps inside a task are bite-sized — one action each — and contain the actual test code, the actual
implementation, and the actual commit message. A step that describes what to do without showing how
is a plan failure.

**Mark a task `Needs human: yes`** when its correctness depends on behaviour a person has to
observe. The runner will implement it, verify it, and leave the issue open.

## Phase 7 — mechanical check, then a second outside voice

Two passes, in this order, because the cheap one must never be paid for by the dear one.

### 7a — the mechanical pass, run by this session

```sh
sh <skill-dir>/validate-plan.sh docs/plans/<the-plan>.md <project-root>
```

Exit 0 and no output, or fix what it names. Then read for what a script cannot see:

- placeholders — `TBD`, `TODO`, "add error handling", "similar to Task N"
- names that drift between tasks — `clearLayers` in one, `clearFullLayers` in another
- a step that says *what* to do without showing *how*

Fix inline. This pass is cheap and catches the mechanical faults, so it runs first — sending them to
a sub-agent is paying a reviewer to find a typo.

### 7b — the outside voice, on the design and the plan together

A fresh sub-agent, given the paths to **the design, the plan, and the project's rules**, reading all
three itself. The same three brief rules as phase 4 apply unchanged, and the third matters most
here: the plan is long, and a brief that summarises it is handing over the anchoring again.

It answers questions this session structurally cannot:

| Question | Why the writer cannot answer it |
|---|---|
| Does the plan actually implement the design? Which section has no task? | The writer knows what they *meant* each task to cover, and reads that intent back into the text |
| Could an engineer with no context execute this task as written? | The writer has all the context. Everything looks sufficient from there |
| Is anything presented as settled that was never proven? | A claim the writer believes reads like a fact to the writer |
| What is missing entirely? | The hardest one, and unreachable from inside the plan's own frame |

**Findings go to the operator, unchanged.** The skill does not adjudicate them — deciding a reviewer
is wrong is a decision, and decisions belong to phase 8.

### When 7b may be skipped

On the **bounded** path, when the plan is one to three tasks and 7a found nothing. Say so out loud
rather than skipping quietly: *"one task, mechanical checks clean, no outside review run."* The
operator can always ask for it. On the architectural path it is never skipped — a design large
enough to need its own document is large enough to be wrong in a way its author cannot see.

### 7c — what happens to a finding

This step governs the findings from **phase 4 as well as 7b**. Getting it wrong wastes both reviews,
and it fails in two symmetric directions:

- **Performative agreement** — accept everything, rewrite to match, and ship something worse,
  because the reviewer had less context *by design* and was never meant to be right about
  everything.
- **Defensive dismissal** — explain why each finding is mistaken. The writer can always do this;
  they hold all the context. The review then cost real money and changed nothing.

The discipline is the one `superpowers:receiving-code-review` states: technical rigour, neither
performative agreement nor blind implementation. Concretely, **every finding gets exactly one of
three dispositions, and each carries an obligation**:

| Disposition | What it requires |
|---|---|
| **Accepted** | The change, made. And if it touches an assumption a PoC settled, **that PoC is re-run**, not reasoned about |
| **Rejected** | **Evidence, not argument** — a file, a measured output, or the line in the design the reviewer missed. "The reviewer lacked context" is not evidence; if the context was missing, the design failed to state it, and that is itself a finding |
| **Undecided** | Handed to the operator as an open question, and the design's *Still unverified* gains a row |

The asymmetry is deliberate: **a rejection needs evidence, an acceptance needs a re-run.** Both
directions cost something, so neither is the lazy default.

### 7d — at most one re-review, then stop

If accepted findings changed the design or the plan materially, run the affected review **once more**
with a fresh agent, scoped to what changed.

**One extra round, never a loop.** If a second round still returns material findings, that is not a
document to keep polishing — it is a signal the design is not ready, and the right move is to say so
to the operator rather than iterate toward a document nobody has read from outside in three rounds.
This is the same bound, and the same reason, as `max_rounds` in the runtime pipeline.

Write the ledger to `docs/design/YYYY-MM-DD-<topic>-review.md`:

```markdown
| # | Lens | Finding | Disposition | Evidence or change |
|---|---|---|---|---|
| 1 | engineering | … | Accepted | commit-less edit to §…; poc/…/probe.sh re-run, still green |
| 2 | product | … | Rejected | `docs/reference/…md:41` already measures this |
| 3 | 7b | … | Undecided | added to *Still unverified* |
```

This file is what makes the review auditable instead of theatre. A review whose findings were all
rejected, with no evidence recorded, did not happen.

## Phase 8 — stop

State four things, in this order, and then stop.

**1. What was written.** Every path, including the PoCs and the review ledger.

**2. What an outside voice found, and what was done about it.** Not "reviewed, all good" — the
counts and the disagreements, because those are what the operator is actually being asked to
approve:

> Two lenses and one plan review: 9 findings. 6 accepted and applied, 2 rejected with evidence
> (ledger rows 3 and 7), 1 undecided and now in *Still unverified*.

**3. What is still unproven.** Read the design's *Still unverified* section out loud. An operator
approving a design is approving its open questions too, and those are the easiest thing in a long
document to approve by not noticing.

**4. That nothing was committed, and that committing is the approval:**

> Nothing is committed. Committing these files is the approval, and `autopilot-deliver` checks for
> it mechanically before it will shard anything.

Then stop. Do not offer to commit. Do not invoke `autopilot-deliver`.

**If a finding was rejected, say which and on what evidence.** A rejection the operator never saw is
a decision the skill made on its own, and §2.4 does not stop applying because a person is in the
room.
```

- [ ] **Step 2: Verify the frontmatter and structure**

```sh
head -4 skills/autopilot-planning/SKILL.md
grep -c '^## Phase' skills/autopilot-planning/SKILL.md
grep -c 'none given\|no particular idea' skills/autopilot-planning/SKILL.md
grep -c '^### 3[a-d]' skills/autopilot-planning/SKILL.md
grep -c 'docs/design/poc/' skills/autopilot-planning/SKILL.md
grep -c 'sub-agent' skills/autopilot-planning/SKILL.md
grep -c 'Accepted\|Rejected\|Undecided' skills/autopilot-planning/SKILL.md
```

Expected: the first four lines show `---`, `name: autopilot-planning`, a `description:` line, `---`;
then `9`, one per phase; then at least `1`, proving the no-idea branch of phase 0 survived; then `4`,
the sub-steps of phase 3; then at least `1`, proving the PoC location survived; then at least `3`,
proving the outside-voice reviews at phases 4 and 7b survived; then at least `3`, proving the triage
dispositions at 7c survived — a review with no disposition rule is a review whose findings evaporate.

The last three assertions exist because those are the parts most likely to be quietly dropped while
writing. The no-idea branch reads like an edge case and is the entry point for every morning the
operator has time and no particular plan. Research and PoC read like ceremony and are the two steps
that stop an unchecked claim reaching a runner that cannot ask.

- [ ] **Step 3: Commit**

```sh
git add skills/autopilot-planning/SKILL.md
git commit -m "feat: add the autopilot-planning skill

The chain started at a queue. Deliver removed the precondition that issues must
exist; the plan those issues implement still had to arrive from outside.

It writes and stops. Deliver's phase-1 commit gate is the only approval a tired
operator cannot wave through, and a skill that commits its own plan deletes it."
```

---

### Task 7: `autopilot-deliver` reads the structured breakdown

- **Done when:** deliver's phase 3 transcribes the six fields instead of interpreting prose, and
  phase 1 runs the validator as part of the commit gate.
- **Verify:** `grep -c 'validate-plan.sh' skills/autopilot-deliver/SKILL.md` → at least 1; `grep -c 'Needs human' skills/autopilot-deliver/SKILL.md` → at least 1
- **Intent:** `docs/design/2026-08-16-multi-harness-role-pipeline-design.md` `skills/autopilot-deliver/SKILL.md`
- **Depends on:** Task 6
- **Tier:** standard
- **Needs human:** no

**Files:**
- Modify: `skills/autopilot-deliver/SKILL.md`

- [ ] **Step 1: Extend the commit gate in phase 1**

In the *Flow* block, replace the phase 1 lines with:

```
1  ▮ COMMIT GATE — checked mechanically, never asked
     git ls-files --error-unmatch <plan>   must be tracked
     git status --porcelain <plan>         must be empty
     validate-plan.sh <plan> <root>        must exit 0
     fail → stop and name what is missing. No override.
```

- [ ] **Step 2: Replace phase 3 in the Flow block**

```
3  Shard — transcribe, do not interpret
     each task in the plan becomes one issue, field by field:
       heading    → issue title
       Done when  → the done-when section of the body
       Verify     → the verify block
       Intent     → the Intent: line
       Depends on → Depends on #<num>, resolved in phase 5
       Tier       → label tier:<name>
       Needs human→ label needs-human
     also write .autopilot/proposed-issues.json (gitignored) for crash recovery
```

- [ ] **Step 3: Replace the *Issue format* section's last paragraph**

The section currently explains `model/effort`. Replace that sentence with:

```markdown
The tier comes from the task's **Tier** field and becomes a `tier:<name>` label, where the name must
be one the project's ladder declares in `config.json`. The `model:` and `effort:` labels are gone;
the ladder in `.autopilot/tiers.local.json` binds a tier name to a harness and a model.
```

- [ ] **Step 4: Verify**

```sh
grep -c 'validate-plan.sh' skills/autopilot-deliver/SKILL.md
grep -c 'Needs human' skills/autopilot-deliver/SKILL.md
```

Expected: both at least `1`.

- [ ] **Step 5: Commit**

```sh
git add skills/autopilot-deliver/SKILL.md
git commit -m "feat: shard by transcription instead of interpretation

Phase 3 read a plan as prose and turned it into issues by judgement. Judgement
is where a step quietly loses its Intent line and becomes a task the runner
refuses at 3 a.m. Six named fields make sharding a copy.

The commit gate gains the validator, so a malformed plan stops at the gate
rather than at the first issue that fails."
```

---

### Task 8: Plugin wiring and the version bump

- **Done when:** the plugin manifests list three skills, the version is bumped consistently, and the
  whole suite passes.
- **Verify:** `sh tests/run.sh` → `ALL PASS`
- **Intent:** `docs/design/2026-08-16-multi-harness-role-pipeline-design.md` `.claude-plugin/plugin.json`
- **Depends on:** Task 7
- **Tier:** light
- **Needs human:** no

**Files:**
- Modify: `.claude-plugin/plugin.json`, `.claude-plugin/marketplace.json`, `runner/VERSION`,
  `tests/test_plugin.sh`, `README.md`

- [ ] **Step 1: Extend `tests/test_plugin.sh`**

Append before `finish`:

```sh
for s in autopilot-planning autopilot-deliver autopilot-review; do
    assert_eq "1" "$([ -f "$REPO_ROOT/skills/$s/SKILL.md" ] && echo 1 || echo 0)" \
        "the $s skill exists"
    assert_contains "$(head -3 "$REPO_ROOT/skills/$s/SKILL.md")" "name: $s" \
        "$s declares its name in frontmatter"
done
```

- [ ] **Step 2: Run it and confirm it fails only where expected**

```sh
sh tests/test_plugin.sh
```

Expected: the three `autopilot-planning` assertions pass if Task 6 landed; all six pass. If any
fail, the frontmatter name does not match the directory name — fix the skill, not the test.

- [ ] **Step 3: Bump the version in all three places**

`runner/VERSION` is the source; `test_plugin.sh` already asserts the manifests agree with it.

```sh
echo "0.3.0" > runner/VERSION
```

Then set `.version` to `0.3.0` in `.claude-plugin/plugin.json`, and `.plugins[0].version` to
`0.3.0` in `.claude-plugin/marketplace.json`.

- [ ] **Step 4: Update `README.md`**

Replace the sentence naming two skills with one naming three, in the order they run:

```markdown
The plugin ships three skills — `autopilot-planning`, `autopilot-deliver`, and `autopilot-review` —
and `runner/deploy.sh` copies `runner/` to a stable path outside the version-stamped plugin cache.
```

- [ ] **Step 5: Run the full suite**

```sh
shellcheck -s sh skills/autopilot-planning/validate-plan.sh && sh tests/run.sh
```

Expected: no shellcheck output, then `ALL PASS`.

- [ ] **Step 6: Commit**

```sh
git add .claude-plugin/ runner/VERSION tests/test_plugin.sh README.md
git commit -m "chore: ship the planning skill as part of the plugin

Version bumped in one place; the manifests are asserted against runner/VERSION
so a deployed copy and the plugin cannot disagree about what they are."
```

---

### Task 9: Exercise it against a real repository

- **Done when:** the skill has been run end to end against a throwaway repository, producing a
  design, a documentation set, and a plan that `validate-plan.sh` accepts and that
  `autopilot-deliver` shards without a question.
- **Verify:** in the throwaway repository, `validate-plan.sh` exits 0 and `gh issue list --label autopilot --json number,labels` shows one issue per task, each carrying a `tier:` label
- **Intent:** `docs/design/2026-08-16-multi-harness-role-pipeline-design.md` `docs/reference/observed-behaviour.md`
- **Depends on:** Task 8
- **Tier:** deep
- **Needs human:** yes

`Needs human: yes` — this is the Definition-of-Done clause that says *exercised against a real
repository, not only in tests*, and no automated check substitutes for it.

**Files:**
- Modify: `docs/reference/observed-behaviour.md`, `docs/README.md`, `docs/product/open-items.md`

- [ ] **Step 1: Create a throwaway repository with a real remote**

```sh
gh repo create autopilot-planning-probe --private --clone
cd autopilot-planning-probe
git commit -q --allow-empty -m "seed"
git push -q -u origin HEAD
```

- [ ] **Step 2: Run `autopilot-planning` against it**

Give it a small, real idea — something with three or four tasks, such as *"a script that reports
which files in this repository have no test"*. Take it through all eight phases.

Confirm at phase 8 that **nothing was committed**. This is the behaviour most likely to be wrong,
because committing is the natural end of writing files.

```sh
git status --porcelain
```

Expected: the written files appear as untracked or modified, none staged, none committed.

- [ ] **Step 3: Commit the plan as the operator, then run `autopilot-deliver`**

```sh
git add -A && git commit -m "docs: the plan" && git push -q
```

Then run `autopilot-deliver` and confirm phase 1 passes the gate, phase 3 transcribes rather than
asks about each field, and phase 5 creates the issues.

- [ ] **Step 4: Confirm the issues carry what the runner needs**

```sh
gh issue list --label autopilot --json number,title,labels
gh issue view 1 --json body -q .body | grep -i '^Intent:'
```

Expected: one issue per task; every issue carries a `tier:` label; the `Intent:` line is present and
names files that exist in that repository.

- [ ] **Step 5: Record what was learned**

Add a dated section to `docs/reference/observed-behaviour.md` describing what actually happened —
including anything that went wrong, which is the point of the file. If nothing went wrong, say that
and say what was run, so the next reader knows the claim was tested rather than assumed.

Update `docs/README.md`'s status row for this plan to `✅ Delivered`, and move any question this
raised into `docs/product/open-items.md`.

- [ ] **Step 6: Delete the throwaway repository**

```sh
cd .. && gh repo delete autopilot-planning-probe --yes && rm -rf autopilot-planning-probe
```

- [ ] **Step 7: Commit**

```sh
git add docs/
git commit -m "docs: record the planning skill's first end-to-end run

Written after running it against a real repository with a real remote, not
after reading the skill and believing it. The Definition of Done asks for
exactly this, and it is the clause this repository has skipped before."
```

---

## Verification

Per task, the `Verify` field above. For the plan as a whole:

| Check | Command | Passing looks like |
|---|---|---|
| Unit and integration suite | `sh tests/run.sh` | `ALL PASS`, and three more test files than before |
| Shell lint | `shellcheck -s sh skills/autopilot-planning/validate-plan.sh` | no output |
| The plan satisfies its own rule | `sh skills/autopilot-planning/validate-plan.sh docs/plans/2026-08-16-planning-skill.md .` | exit 0, no output |
| The boundary is intact | `grep -rl 'skills/' runner/run-once.sh runner/lib/` | no output |
| Exercised for real | Task 9 | issues created in a real repository, each with `Intent:` and a `tier:` label |

**"The tests pass" is not sufficient.** Task 9 is the claim that this works, and it is the one that
has to be run and observed rather than argued.

## Documentation to update

- [ ] `AGENTS.md` — created from `CLAUDE.md`, gains the §5 sync table (Task 1)
- [ ] `CLAUDE.md` — reduced to a pointer (Task 1)
- [ ] `docs/README.md` — created, with a status row per document (Task 2), updated at Task 9
- [ ] `docs/product/open-items.md` — created (Task 2), updated at Task 9
- [ ] `docs/plans/_template.md` — structured work breakdown (Task 4)
- [ ] `docs/reference/observed-behaviour.md` — the end-to-end run (Task 9)
- [ ] `README.md` — three skills, in the order they run (Task 8)
- [ ] `skills/autopilot-deliver/SKILL.md` — phases 1 and 3 (Task 7)

## Risks

| Risk | Level | Mitigation |
|---|---|---|
| A pointer `CLAUDE.md` adds an indirection a cheap-tier model may not follow, so the rules go unread | **High** | Task 1's pointer is four lines and its first sentence is an instruction to read `AGENTS.md`. Task 9 observes whether a real run followed it. If it does not, the finding goes in `observed-behaviour.md` and decision 20 is revisited by ADR — not by quietly copying the rules back |
| The validator's `Intent:` path check is too strict and rejects a legitimate plan | Medium | It skips any token containing a character outside `[A-Za-z0-9._/-]`, and skips `—`. Task 4 runs it against this plan, which is the hardest real case available |
| The skill writes a documentation set over a project that already has one | Medium | Phase 5 acts only on what is missing, and reports drift rather than rewriting. Stated in the skill and in *Out of scope* |
| Three skills is one too many to keep straight, and the operator invokes the wrong one | Low | Each `description:` names when to use it and what precedes it. `README.md` lists them in run order |
| `git mv CLAUDE.md AGENTS.md` loses history for readers who search for `CLAUDE.md` | Low | `git mv` preserves history and the pointer file keeps the name discoverable |

## Out of scope

- **Anything in Rust.** The runner rewrite is plan 2 of four, and no part of it starts here.
- **Decision 23 — the implementer writing the documentation its step binds.** That lives in the
  role prompt templates, which are built in plan 2. This plan supplies the other half: the plan's
  *Documentation to update* list, which is what tells the implementer the obligation exists.
- **The tier ladder and the harness preflight.** `autopilot-deliver` phase 2 gains those in plan 3;
  Task 7 changes only how phase 3 reads a plan.
- **The journal and the dashboard.** Plan 4.
- **Migrating an existing project's documentation wholesale.** Phase 5 scaffolds what is missing and
  reports what has drifted; rewriting a large existing set is a task for a plan of its own.
- **Re-planning after a design changes.** Reconciling a revised plan against issues that already
  exist is a real problem and a different one.
- **Replacing `autopilot-review`.** It is unchanged by this plan.
