# Provider preflight in `autopilot-deliver` — implementation plan

> **Type:** Plan · **Date:** 2026-08-16 · **Status:** Awaiting approval · **Rewritten 2026-08-19 against decisions 26–42**
> **Prerequisite:** docs/plans/2026-08-16-planning-skill.md, docs/plans/2026-08-16-rust-runner.md
> **Scope:** Sub-project D of the multi-harness design — detecting which harnesses and models this
> machine can actually reach, proposing a tier ladder from that, and refusing to proceed when a
> named tier does not resolve.

> **For agentic workers:** Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `autopilot-deliver` establish, with the operator, a tier ladder bound to harnesses and
models that exist on this machine — and stop, naming what is missing, when one does not.

**Architecture:** `autopilot preflight` is a read-only subcommand of the Rust binary that reports
what every adapter's `check()` and `list_models()` return, as JSON. The skill reads that, proposes a
ladder, the operator confirms, and the skill writes the two config layers. Nothing is detected twice
in two languages.

**Tech Stack:** Rust (the subcommand, built in plan 1), Markdown (the skill), `jq` for the skill's
own reading of the JSON. No new crate.

**Spec:** [`docs/design/2026-08-16-multi-harness-role-pipeline-design.md`](../design/2026-08-16-multi-harness-role-pipeline-design.md)
— decisions 8, 9, 10, 21, 38 and 42, and the sections *Preflight in `autopilot-deliver`* and *The
tier ladder, in two layers*.

## Global Constraints

- **Detect, propose, the operator confirms, then write.** Never decide silently.
- **The skill does not reimplement detection.** `check()` and `list_models()` live in the adapters;
  the skill reads one JSON document.
- `config.json` (tier **names**) is committed; `.autopilot/tiers.local.json` (the **bindings**) is
  gitignored. Decision 10.
- **A tier that does not resolve stops the flow**, naming the harness or model that is missing. No
  override.
- Rust gates as in plan 1: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, no
  `unwrap`/`expect` on reachable paths.

## Context

The tier ladder is the piece of configuration with no safe default. `verify` at least fails loudly
when it is wrong; a tier bound to a harness that is not installed produces a wake that stands down
every night, which is indistinguishable from a quiet queue. It also describes **the machine** rather
than the project, so it is the piece most likely to be wrong.

Decision 26 had deferred this plan behind a second-harness cost measurement. **Decision 42 (operator
override, 2026-08-19) authorises building it now** as part of the four-plan build, with the risk of
designing the ladder from a sample of one measured harness accepted knowingly.

## What we already know

- **`check()` and `list_models()` exist** on the `Harness` trait as of plan 1 task 8, and the
  adapters implement them. This plan exposes them; it does not write detection logic.
- **Measured traps the preflight must not fall into** (`docs/reference/observed-behaviour.md`):
  - `pi auth check --provider P --json` reports readiness in its JSON `status`; the exit code alone
    is not the whole story.
  - `pi -p --mode json` **exits 0 even when the model errors** — the error is `"stopReason":"error"`
    inside the stream. Recorded 2026-08-19.
  - `opencode models` lists reachable models, and on the development machine fails on an invalid
    user config with the error on stderr and exit 1. A preflight that reports "no models" there
    would be wrong in a way the operator cannot act on; it must report the error verbatim.
  - `opencode providers list` lists credentials and exits 0.
- **The skill's phase 2 already has the shape this extends.** `skills/autopilot-deliver/SKILL.md`
  detects the project type, proposes `verify`, and the operator confirms.
- **`config.json` is tracked**, so an uncommitted edit is reverted by the first rejection path. The
  local bindings file is gitignored and does not share that hazard.

## Approach

Four tasks. The subcommand first, because the skill has nothing to read until it exists.

**Files created** — `runner/src/preflight.rs`, `runner/tests/preflight.rs`

**Files modified** — `runner/src/main.rs`, `skills/autopilot-deliver/SKILL.md`,
`docs/guides/install.md`

---

## Work breakdown

### Task 1: `autopilot preflight` — report what this machine can reach

- **Done when:** the subcommand prints one JSON document describing every adapter, and an adapter
  whose CLI errors reports the error verbatim rather than an empty model list.
- **Verify:** `cd runner && cargo test preflight` → all green; with a stubbed failing `opencode`, the report's `opencode.error` equals the stub's stderr
- **Intent:** docs/design/2026-08-16-multi-harness-role-pipeline-design.md
- **Depends on:** —
- **Tier:** standard
- **Needs human:** no

**Files:**
- Create: `runner/src/preflight.rs`, `runner/tests/preflight.rs`
- Modify: `runner/src/main.rs`

**Interfaces:**
- `preflight::run(project) -> PreflightReport` printing to stdout:

```json
{
  "harnesses": [
    {"name":"claude","available":true,"models":[],"error":null,"proven":true},
    {"name":"pi","available":true,"models":["deepseek/deepseek-v4-flash","deepseek/deepseek-v4-pro"],"error":null,"proven":true},
    {"name":"opencode","available":false,"models":[],"error":"Configuration is invalid at …","proven":false}
  ],
  "tiers_declared": ["light","standard","deep"],
  "bindings_present": ["standard","deep"],
  "unresolved": ["light"]
}
```

`unresolved` is the field the skill and the operator act on. `proven` is false for any harness that
has never produced a real run transcript. A binding whose harness is unavailable, or whose model is
absent from a non-empty `models` list, is unresolved.

- [ ] **Step 1: Write the failing tests**: an adapter whose CLI errors reports the error, not an
  empty list; a declared tier with no binding appears in `unresolved`; a binding naming a model the
  harness cannot reach is unresolved; one adapter's failure never aborts the whole report.
- [ ] **Step 2: Run and confirm failure.**
- [ ] **Step 3: Implement** by iterating `harness::registry()` and calling `check(model)` and
  `list_models()`.
- [ ] **Step 4: Run tests and gates.**
- [ ] **Step 5: Commit.**

### Task 2: `guard_tiers` and preflight share one code path

- **Done when:** `guard_tiers` calls the same `check()` the preflight reports, and a test proves a
  tier that preflight calls unresolved also stands the runner down.
- **Verify:** `cd runner && cargo test guard` → all green, including the agreement test
- **Intent:** docs/design/2026-08-16-multi-harness-role-pipeline-design.md
- **Depends on:** Task 1
- **Tier:** standard
- **Needs human:** no

**Files:**
- Modify: `runner/src/guard.rs` (or `preflight.rs` if the shared helper lives there)

**Interfaces:**
- One function both sides call; a test asserts preflight's `unresolved` implies `guard_tiers` stands down.

- [ ] **Step 1: Write the agreement test.**
- [ ] **Step 2: Run and confirm failure.**
- [ ] **Step 3: Refactor so both call one function.**
- [ ] **Step 4: Run tests, gates.**
- [ ] **Step 5: Commit.**

### Task 3: `autopilot-deliver` phase 2 establishes the ladder

- **Done when:** phase 2 runs preflight, proposes a ladder, takes the operator's confirmation, writes
  both config layers, re-checks, and stops naming what is missing when anything is unresolved.
- **Verify:** `grep -c 'autopilot preflight' skills/autopilot-deliver/SKILL.md` → at least 1; `grep -c 'tiers.local.json' skills/autopilot-deliver/SKILL.md` → at least 2
- **Intent:** docs/design/2026-08-16-multi-harness-role-pipeline-design.md skills/autopilot-deliver/SKILL.md
- **Depends on:** Task 1
- **Tier:** standard
- **Needs human:** no

**Files:**
- Modify: `skills/autopilot-deliver/SKILL.md`

**Interfaces:**
- Phase 2 gains the ladder step; a new section "Establishing the tier ladder" states the three
  proposal rules (never propose an unproven harness without saying so; never invent a model name; a
  two-entry ladder means the top tier reviews itself — say so out loud).

- [ ] **Step 1: Replace phase 2 in the Flow block** and add the ladder section.
- [ ] **Step 2: Update the *Prepare steps* checklist.**
- [ ] **Step 3: Verify and commit.**

### Task 4: Phase 3 writes `tier:` labels from the plan

- **Done when:** sharding reads each task's **Tier** field, applies `tier:<name>`, refuses a tier the
  ladder does not declare, and no longer writes `model:` or `effort:`.
- **Verify:** `grep -c 'model:' skills/autopilot-deliver/SKILL.md` → 0; `grep -c 'tier:' skills/autopilot-deliver/SKILL.md` → at least 1
- **Intent:** docs/design/2026-08-16-multi-harness-role-pipeline-design.md skills/autopilot-deliver/SKILL.md
- **Depends on:** Task 3
- **Tier:** standard
- **Needs human:** no

**Files:**
- Modify: `skills/autopilot-deliver/SKILL.md`

**Interfaces:**
- The *Issue format* tier paragraph states a `Tier` naming something outside `config.tiers` is a
  **plan error, not a label to create**; the label-creation list adds `tier:<name>` per declared tier.

- [ ] **Step 1: Replace the tier paragraph and add the label list.**
- [ ] **Step 2: Verify and commit.**

---

## Verification

| Check | Command | Passing looks like |
|---|---|---|
| Unit | `cd runner && cargo test preflight guard` | all green |
| Gates | `cargo clippy --all-targets -- -D warnings && cargo fmt --check` | no output |
| One detector | Task 2's test | preflight and `guard_tiers` cannot disagree |
| Skill updated | `grep -c 'model:' skills/autopilot-deliver/SKILL.md` | `0` |

## Documentation to update

- [ ] `skills/autopilot-deliver/SKILL.md` — phases 2 and 3, the ladder section, prepare steps (Tasks 3, 4)
- [ ] `docs/guides/install.md` — the ladder, the two config layers, the proven-adapter table (Task 3)
- [ ] `docs/reference/observed-behaviour.md` — the `pi`/`opencode` measurements (recorded in plan 1 task 16)

## Risks

| Risk | Level | Mitigation |
|---|---|---|
| The skill and `guard_tiers` disagree, so the ladder passes preflight and stands down nightly | **High** | Task 2 makes them one code path and asserts it |
| A two-entry ladder means the top tier reviews its own tier's work | Medium | The skill says so out loud when proposing |
| The operator confirms a ladder naming an unproven adapter | Medium | `proven: false` is carried through preflight to the proposal, and the skill must say it |
| Preflight is slow enough that the skill feels stuck | Low | It runs `--version`-class commands per harness; cache within one skill session, never across |

## Out of scope

- **Fixing the operator's machine.** The preflight reports; repairing an `opencode` config or
  starting an LM Studio server is the operator's action.
- **Installing harnesses.** Detecting them is enough.
- **The `codex` adapter.** Not installed on the development machine.
- **Re-establishing the ladder from inside a scheduled run.** `guard_tiers` reports and stands down;
  rebinding a tier is a decision, and §2.4 reserves decisions for a person.
