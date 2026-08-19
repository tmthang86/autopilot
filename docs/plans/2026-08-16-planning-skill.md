# The planning skill and the documentation set — implementation plan

> **Type:** Plan · **Date:** 2026-08-16 · **Status:** Awaiting approval · **Rewritten 2026-08-19 against decisions 26–42**
> **Scope:** Sub-project F of the multi-harness design — `autopilot-planning`, the documentation
> convention it installs, and the mechanical check that makes a plan a contract rather than prose.

> **For agentic workers:** Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give autopilot a third skill that turns an idea into a committed plan the delivery chain
can shard mechanically, and give this repository the documentation convention that skill installs
everywhere else.

**Architecture:** One new skill (Markdown), one POSIX shell validator that checks a plan's steps
carry every field the delivery chain consumes, a set of templates the skill installs into target
projects, and the sync table that keeps this repository's own documentation true. Nothing under
`runner/` is touched.

**Tech Stack:** POSIX shell (`sh`), Markdown, `git`, `gh`, `jq`, `shellcheck`. No new dependency.

**Spec:** [`docs/design/2026-08-16-multi-harness-role-pipeline-design.md`](../design/2026-08-16-multi-harness-role-pipeline-design.md)
— decisions 17–23 and 39–42, and the section *The skill trio, and `autopilot-planning`*.

## Global Constraints

- **POSIX shell only** in this plan. The Rust rewrite is a separate plan and must not begin here.
- `shellcheck` clean before every commit.
- Tests run against a **real throwaway git repository**, never mocks of git. `gh` and the agent CLI
  are stubbed with fixture scripts on `PATH`.
- **Nothing under `runner/` changes in this plan.**
- Conventional Commits. One commit is one coherent change **including its documentation**.
- **`CLAUDE.md` stays the single source of agent rules in this repository** (decision 39, which
  reopened decision 20). `AGENTS.md` is a pointer to it. The convention the skill installs in
  *target* projects remains `AGENTS.md`-canonical, with the caveat named in the skill.
- A plan step carries a goal in its heading plus six bold fields: `Done when`, `Verify`, `Intent`,
  `Depends on`, `Tier`, `Needs human` (decision 21).
- **In a target project, `autopilot-planning` writes but never commits** (decision 18). This plan
  implements that behaviour; it does not apply it to this repository, where the operator is running
  the plan directly.

---

> **Resolution of the review's validator objection, 2026-08-19.** The engineering review's verdict
> said this plan "needs re-approval for its validator change". The change in question is the one the
> earlier amendment already described: the validator's `Intent:`-path existence check must resolve a
> path against the repository **plus** every path any task in this plan, or any plan named as a
> `Prerequisite:`, says it will create. Without that, the validator rejects every plan that cites a
> file it is about to create — including this one. That change is **kept as specified and is now
> resolved**: Task 3 implements it, Task 3's tests assert it, and Task 4 runs the validator against
> this plan as the hardest real input. No further approval gate is left open.

## Context

The product starts at a queue. `autopilot-deliver` turns an approved plan into issues and
`autopilot-review` reads what came back, but the plan itself has to arrive from somewhere outside
anything this repository provides. `autopilot-deliver` phase 3 reads a plan as prose and interprets
it into issues; interpretation is where a step quietly loses its `Intent:` line — and the runner
refuses such a task at 3 a.m., after the operator has gone to bed. This plan closes the front of the
chain and makes the plan itself checkable.

## What we already know

- **`autopilot-deliver`'s commit gate already exists and is mechanical.** Phase 1 runs
  `git ls-files --error-unmatch <plan>` and `git status --porcelain <plan>`, and stops with no
  override.
- **The runner refuses a task with no `Intent:` line before spending a token.** ADR-0005.
- **`AGENTS.md` is a cross-tool standard**, but a `claude` session loads the project's `CLAUDE.md`
  automatically (`observed-behaviour.md`, 2026-08-15). Decision 39 therefore keeps `CLAUDE.md`
  canonical here: a pointer would trade measured automatic rule delivery for an argument about
  standards.
- **The test harness API is fixed.** `tests/harness.sh` provides `assert_eq`, `assert_contains`,
  `make_repo`, `stub_bin`, `finish`, and exports `REPO_ROOT`, `RUNNER_ROOT`, `TEST_TMP`. It does
  **not** set `-e`.
- **Two shell traps this repository has already paid for** (`observed-behaviour.md`): a bare
  `x=$(false)` under `set -eu` aborts even inside an `if` body; command substitution and pipelines
  run in a subshell, so a counter incremented inside `cmd | while read` is lost. The validator must
  redirect (`done < "$file"`), never pipe.
- **The four plans (including this one) now use the six-field form**, so the validator runs against
  them as its first real inputs.

## Approach

Nine tasks. Tasks 1–2 make this repository carry the convention it is about to install elsewhere;
tasks 3–4 build the mechanical check and apply it to this repository's own template; tasks 5–6 build
the skill; tasks 7–8 wire it into the delivery chain and the plugin; task 9 exercises the whole thing
against a real repository.

**Files created**

| Path | Responsibility |
|---|---|
| `AGENTS.md` | A pointer to `CLAUDE.md` (the cross-tool entry point) |
| `docs/README.md` | The documentation map and the status table for every document |
| `docs/product/open-items.md` | Questions with no answer yet, and accepted technical debt |
| `skills/autopilot-planning/SKILL.md` | The eight-phase skill |
| `skills/autopilot-planning/validate-plan.sh` | The mechanical six-field check |
| `skills/autopilot-planning/templates/*.tmpl` | What the skill installs into a target project |
| `tests/test_agents_md.sh`, `tests/test_docs_status.sh`, `tests/test_plan_format.sh` | The three new test files |

**Files modified**

| Path | Change |
|---|---|
| `CLAUDE.md` | Gains the §5 sync table; stays canonical |
| `docs/plans/_template.md` | Work breakdown becomes the six-field structured form |
| `skills/autopilot-deliver/SKILL.md` | Phase 1 runs the validator; phase 3 reads the structured breakdown |
| `tests/run.sh` | Runs the three new test files |
| `.claude-plugin/plugin.json`, `.claude-plugin/marketplace.json`, `runner/VERSION` | Version bump (shared with plan 1) |

---

## Work breakdown

### Task 1: `CLAUDE.md` stays canonical, gains the sync table; `AGENTS.md` becomes the pointer

- **Done when:** `CLAUDE.md` carries the rules including the binding sync table; `AGENTS.md` is at
  most twelve lines and names `CLAUDE.md`; `tests/test_agents_md.sh` passes.
- **Verify:** `sh tests/test_agents_md.sh` → 5 assertions, `0 failed`
- **Intent:** docs/design/2026-08-16-multi-harness-role-pipeline-design.md docs/reference/observed-behaviour.md
- **Depends on:** —
- **Tier:** standard
- **Needs human:** no

**Files:**
- Create: `AGENTS.md`, `tests/test_agents_md.sh`
- Modify: `CLAUDE.md`, `tests/run.sh`

**Interfaces:**
- `CLAUDE.md` remains the path every later task and every template refers to for rules. It gains the
  §5 sync table ("When you change… You must update…") originally planned for `AGENTS.md`.

- [ ] **Step 1: Write the failing test** — `AGENTS.md` exists and names `CLAUDE.md`; `AGENTS.md` is a
  pointer, not a copy (≤12 lines); `CLAUDE.md` carries "runs unsupervised" and the sync table.
- [ ] **Step 2: Run it and confirm it fails.**
- [ ] **Step 3: Add the sync table to `CLAUDE.md`** as a new §5, renumbering the sections after it.
- [ ] **Step 4: Write `AGENTS.md`** as a pointer naming `CLAUDE.md` — the reverse of decision 20, per
  decision 39, because a `claude` session loads `CLAUDE.md` automatically.
- [ ] **Step 5: Register the test and run the suite.**
- [ ] **Step 6: Commit.**

### Task 2: The tracking artifacts — the documentation map and open items

- **Done when:** `docs/README.md` holds a status row for every Markdown file under `docs/`, and
  `docs/product/open-items.md` records the items already known to be unverified.
- **Verify:** `sh tests/test_docs_status.sh` → every document listed, `0 failed`
- **Intent:** docs/design/2026-08-16-multi-harness-role-pipeline-design.md
- **Depends on:** Task 1
- **Tier:** standard
- **Needs human:** no

**Files:**
- Create: `docs/README.md`, `docs/product/open-items.md`, `tests/test_docs_status.sh`
- Modify: `tests/run.sh`

**Interfaces:**
- `docs/README.md` — the file the status test and the skill's phase 5 both read.

- [ ] **Step 1: Write the failing test** that walks the real `docs/` tree (redirected, never piped).
- [ ] **Step 2: Run it and confirm it fails.**
- [ ] **Step 3: Write `docs/README.md`** with a status row for every document, and
  `docs/product/open-items.md` seeded from the design's *Still unverified* section.
- [ ] **Step 4: Run the test and the suite.**
- [ ] **Step 5: Commit.**

### Task 3: The plan validator

- **Done when:** `validate-plan.sh` exits 0 on a well-formed plan, exits 1 naming every missing field
  on a malformed one, and reports an `Intent:` path that does not exist — unless a task or a
  prerequisite plan says it will create it.
- **Verify:** `sh tests/test_plan_format.sh` → 16 assertions, `0 failed`; `shellcheck -s sh skills/autopilot-planning/validate-plan.sh` → clean
- **Intent:** docs/design/2026-08-16-multi-harness-role-pipeline-design.md
- **Depends on:** —
- **Tier:** standard
- **Needs human:** no

**Files:**
- Create: `skills/autopilot-planning/validate-plan.sh`, `tests/test_plan_format.sh`
- Modify: `tests/run.sh`

**Interfaces:**
- `validate-plan.sh <plan.md> [repo-root]` — exit 0 clean, exit 1 with one line per problem, exit 2
  if the plan file does not exist. Phase 7 of the skill and phase 1 of deliver invoke it.

- [ ] **Step 1: Write the failing test** — the sixteen assertions from the amended plan, including
  the two that resolved the review objection: a path a later task creates is not a typo, and a path a
  prerequisite plan creates is accepted; plus the fenced-example cases and this plan validating
  itself.
- [ ] **Step 2: Run it and confirm it fails.**
- [ ] **Step 3: Write the validator.** It collects every problem before exiting; resolves `Intent:`
  paths against the repository plus everything `- Create:`/`Files created` rows say will exist;
  counts fence depth so a nested example plan is never reported; redirects, never pipes.
- [ ] **Step 4: Make it executable, run the test, run `shellcheck`.**
- [ ] **Step 5: Register the test and run the suite.**
- [ ] **Step 6: Commit.**

### Task 4: The plan template becomes structured

- **Done when:** `docs/plans/_template.md` shows the six-field task form, and the validator runs clean
  over this very plan.
- **Verify:** `sh skills/autopilot-planning/validate-plan.sh docs/plans/2026-08-16-planning-skill.md .` → exit 0, no output
- **Intent:** docs/design/2026-08-16-multi-harness-role-pipeline-design.md docs/plans/_template.md
- **Depends on:** Task 3
- **Tier:** standard
- **Needs human:** no

**Files:**
- Modify: `docs/plans/_template.md`

**Interfaces:**
- The work-breakdown section carries the six-field form and the consumer table.

- [ ] **Step 1: Replace the work-breakdown section of `docs/plans/_template.md`.**
- [ ] **Step 2: Run the validator against this plan.**
- [ ] **Step 3: Commit.**

### Task 5: The templates the skill installs

- **Done when:** `skills/autopilot-planning/templates/` holds seven templates, and each contains no
  literal `TODO` or `TBD`.
- **Verify:** `ls skills/autopilot-planning/templates/ | wc -l` → 7; `grep -rl 'TODO\|TBD' skills/autopilot-planning/templates/` → no output
- **Intent:** docs/design/2026-08-16-multi-harness-role-pipeline-design.md
- **Depends on:** Task 4
- **Tier:** standard
- **Needs human:** no

**Files:**
- Create: `skills/autopilot-planning/templates/AGENTS.md.tmpl`, `docs-README.md.tmpl`,
  `plan.md.tmpl`, `design.md.tmpl`, `adr.md.tmpl`, `open-items.md.tmpl`, `roadmap.md.tmpl`

**Interfaces:**
- The templates are this repository's own artifacts with its content stripped out, placeholders
  written `{{LIKE_THIS}}`.

- [ ] **Step 1: Copy this repository's own artifacts as the starting templates.**
- [ ] **Step 2: Strip this project's content out of the copied templates.**
- [ ] **Step 3: Write `AGENTS.md.tmpl`** (the canonical rules for a target project), `design.md.tmpl`,
  and `roadmap.md.tmpl`.
- [ ] **Step 4: Verify no placeholder leaked in.**
- [ ] **Step 5: Commit.**

### Task 6: The `autopilot-planning` skill

- **Done when:** `skills/autopilot-planning/SKILL.md` exists with valid frontmatter and describes all
  eight phases, both hard gates, the no-idea branch of phase 0, the four sub-steps of phase 3, the
  fresh-sub-agent reviews at phases 4 and 7b, the three dispositions and the one-re-review bound at
  7c and 7d, and the rule that it commits nothing.
- **Verify:** `head -4 skills/autopilot-planning/SKILL.md` → shows `name:` and `description:`; `grep -c '^## Phase' skills/autopilot-planning/SKILL.md` → 8
- **Intent:** docs/design/2026-08-16-multi-harness-role-pipeline-design.md
- **Depends on:** Task 5
- **Tier:** deep
- **Needs human:** no

**Files:**
- Create: `skills/autopilot-planning/SKILL.md`

**Interfaces:**
- Consumes the templates from Task 5 and `validate-plan.sh` from Task 3 at phase 7. Produces a
  committed-by-the-operator plan in `docs/plans/`.

- [ ] **Step 1: Write the frontmatter and the flow**, including a note that this repository keeps
  `CLAUDE.md` canonical per decision 39 while target projects follow the `AGENTS.md`-canonical
  convention, and the caveat for claude-first projects.
- [ ] **Step 2: Verify the frontmatter and structure.**
- [ ] **Step 3: Commit.**

### Task 7: `autopilot-deliver` reads the structured breakdown

- **Done when:** deliver's phase 3 transcribes the six fields instead of interpreting prose, and
  phase 1 runs the validator as part of the commit gate.
- **Verify:** `grep -c 'validate-plan.sh' skills/autopilot-deliver/SKILL.md` → at least 1; `grep -c 'Needs human' skills/autopilot-deliver/SKILL.md` → at least 1
- **Intent:** docs/design/2026-08-16-multi-harness-role-pipeline-design.md skills/autopilot-deliver/SKILL.md
- **Depends on:** Task 6
- **Tier:** standard
- **Needs human:** no

**Files:**
- Modify: `skills/autopilot-deliver/SKILL.md`

**Interfaces:**
- Phase 1 gains `validate-plan.sh <plan> <root>`; phase 3 transcribes field by field; the *Issue
  format* tier paragraph is replaced (shared with the preflight plan's Task 4 — do not duplicate).

- [ ] **Step 1: Extend the commit gate in phase 1.**
- [ ] **Step 2: Replace phase 3 in the Flow block.**
- [ ] **Step 3: Replace the *Issue format* tier paragraph.**
- [ ] **Step 4: Verify and commit.**

### Task 8: Plugin wiring and the version bump

- **Done when:** the plugin manifests list three skills, the version is bumped consistently, and the
  whole suite passes.
- **Verify:** `sh tests/run.sh` → `ALL PASS`
- **Intent:** .claude-plugin/plugin.json docs/design/2026-08-16-multi-harness-role-pipeline-design.md
- **Depends on:** Task 7
- **Tier:** light
- **Needs human:** no

**Files:**
- Modify: `.claude-plugin/plugin.json`, `.claude-plugin/marketplace.json`, `runner/VERSION`,
  `tests/test_plugin.sh`, `README.md`

**Interfaces:**
- `tests/test_plugin.sh` asserts three skills exist and the manifests agree with `runner/VERSION`.

- [ ] **Step 1: Extend `tests/test_plugin.sh`.**
- [ ] **Step 2: Bump the version in all three places.**
- [ ] **Step 3: Update `README.md`.**
- [ ] **Step 4: Run the full suite.**
- [ ] **Step 5: Commit.**

### Task 9: Exercise it against a real repository

- **Done when:** the skill has been run end to end against a throwaway repository, producing a design,
  a documentation set, and a plan that `validate-plan.sh` accepts and that `autopilot-deliver` shards
  without a question.
- **Verify:** in the throwaway repository, `validate-plan.sh` exits 0 and `gh issue list --label autopilot --json number,labels` shows one issue per task, each carrying a `tier:` label
- **Intent:** docs/design/2026-08-16-multi-harness-role-pipeline-design.md docs/reference/observed-behaviour.md
- **Depends on:** Task 8
- **Tier:** deep
- **Needs human:** yes

`Needs human: yes` — this is the Definition-of-Done clause that says *exercised against a real
repository, not only in tests*. **Deferred in this unattended run**: the skill, validator, templates,
and wiring are delivered and tested; the end-to-end run against a real repository with a real remote
is left for a human, and the throwaway repository is not created or deleted by this run.

- [ ] **Step 1: Create a throwaway repository with a real remote.**
- [ ] **Step 2: Run `autopilot-planning` against it.**
- [ ] **Step 3: Commit the plan as the operator, then run `autopilot-deliver`.**
- [ ] **Step 4: Confirm the issues carry what the runner needs.**
- [ ] **Step 5: Record what was learned and delete the throwaway repository.**
- [ ] **Step 6: Commit the record.**

---

## Verification

| Check | Command | Passing looks like |
|---|---|---|
| Unit and integration suite | `sh tests/run.sh` | `ALL PASS`, and three more test files than before |
| Shell lint | `shellcheck -s sh skills/autopilot-planning/validate-plan.sh` | no output |
| The plan satisfies its own rule | `sh skills/autopilot-planning/validate-plan.sh docs/plans/2026-08-16-planning-skill.md .` | exit 0, no output |
| The boundary is intact | `grep -rl 'skills/' runner/run-once.sh runner/lib/` | no output |
| Exercised for real | Task 9 | issues created in a real repository, each with `Intent:` and a `tier:` label |

## Documentation to update

- [ ] `CLAUDE.md` — gains the §5 sync table; stays canonical (Task 1)
- [ ] `AGENTS.md` — created as a pointer to `CLAUDE.md` (Task 1)
- [ ] `docs/README.md` — created, with a status row per document (Task 2)
- [ ] `docs/product/open-items.md` — created (Task 2)
- [ ] `docs/plans/_template.md` — structured work breakdown (Task 4)
- [ ] `docs/reference/observed-behaviour.md` — the end-to-end run (Task 9)
- [ ] `README.md` — three skills, in the order they run (Task 8)
- [ ] `skills/autopilot-deliver/SKILL.md` — phases 1 and 3 (Task 7)

## Risks

| Risk | Level | Mitigation |
|---|---|---|
| Keeping `CLAUDE.md` canonical while the convention says `AGENTS.md`-canonical confuses later readers | Medium | `AGENTS.md` is a pointer naming `CLAUDE.md`, and the skill states both the convention and the deviation with its reason (decision 39) |
| The validator's `Intent:` path check is too strict and rejects a legitimate plan | Medium | It skips tokens outside `[A-Za-z0-9._/-]` and `—`; Task 4 runs it against this plan, the hardest real case available |
| The skill writes a documentation set over a project that already has one | Medium | Phase 5 acts only on what is missing, and reports drift rather than rewriting |
| Three skills is one too many to keep straight | Low | Each `description:` names when to use it and what precedes it; `README.md` lists them in run order |

## Out of scope

- **Anything in Rust.** The runner rewrite is plan 1 of four.
- **Decision 23 — the implementer writing the documentation its step binds.** That lives in the
  role prompt templates, built in plan 1.
- **The tier ladder and the harness preflight.** `autopilot-deliver` phase 2 gains those in plan 2.
- **The journal and the dashboard.** Plan 3.
- **Migrating an existing project's documentation wholesale.**
- **Re-planning after a design changes.**
- **Replacing `autopilot-review`.** It is unchanged by this plan.
