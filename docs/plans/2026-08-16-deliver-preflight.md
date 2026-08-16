# Provider preflight in `autopilot-deliver` — implementation plan

> **Type:** Plan · **Date:** 2026-08-16 · **Status:** Awaiting approval
> **Prerequisite:** docs/plans/2026-08-16-planning-skill.md, docs/plans/2026-08-16-rust-runner.md
> **Scope:** Sub-project D of the multi-harness design — detecting which harnesses and models this
> machine can actually reach, proposing a tier ladder from that, and refusing to proceed when a
> named tier does not resolve.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `autopilot-deliver` establish, with the operator, a tier ladder bound to harnesses and
models that exist on this machine — and stop, naming what is missing, when one does not.

**Architecture:** Two halves that must agree. `autopilot preflight` is a new read-only subcommand
that reports what every adapter's `available()` and `models()` return, as JSON. The skill reads that,
proposes a ladder, the operator confirms, and the skill writes the two config layers. Nothing is
detected twice in two languages.

**Tech Stack:** Rust (the new subcommand), Markdown (the skill), `jq` for the skill's own reading of
the JSON. No new crate.

**Spec:** [`docs/design/2026-08-16-multi-harness-role-pipeline-design.md`](../design/2026-08-16-multi-harness-role-pipeline-design.md)
— decisions 8, 9, 10 and 21, and the sections *Preflight in `autopilot-deliver`* and *The tier
ladder, in two layers*.

## Global Constraints

- **Detect, propose, the operator confirms, then write.** Never decide silently. This is the rule
  phase 2 already follows for `verify`, and it exists because a wrong `verify` is the only thing
  standing between the agent and a bad commit.
- **The skill does not reimplement detection.** `available()` and `models()` live in the adapters;
  the skill reads one JSON document. Two detectors in two languages would disagree, and the one that
  matters is the one the runner uses at 3 a.m.
- `config.json` (tier **names**) is committed; `.autopilot/tiers.local.json` (the **bindings**) is
  gitignored. Decision 10.
- **A tier that does not resolve stops the flow**, naming the harness or model that is missing. No
  override.
- Rust gates as in plan 2: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, no
  `unwrap`/`expect` on reachable paths.

---

## Context

The tier ladder is the piece of configuration with no safe default. `verify` at least fails loudly
when it is wrong; a tier bound to a harness that is not installed produces a wake that stands down
every night, which is indistinguishable from a quiet queue — the failure shape this repository has
recorded five separate times.

It is also the piece most likely to be wrong, because it describes **the machine** rather than the
project: an LM Studio server that was running when the ladder was written and is off at 3 a.m., a
`pi` credential that expired, a model pulled out of a local runtime.

So it needs two things that do not currently exist: a way to establish it with a person present and
prove every entry resolves, and a way for the runner to notice when it stops resolving. The second
is `guard_tiers`, built in plan 2 task 10. This plan builds the first, and makes them read the same
detector.

## What we already know

- **`available()` and `models()` exist** on the `Harness` trait as of plan 2 task 12, and the
  adapters implement them. This plan exposes them; it does not write detection logic.
- **Measured 2026-08-16, and each is a trap the preflight must not fall into:**
  - `pi auth check --provider P --json` **exits 0 regardless of readiness**. The status is in the
    JSON. Reading the exit code reports a provider with no credentials as ready.
  - `pi` answers `provider_not_found` for `ollama` and `lmstudio` — it cannot reach either. The
    only measured route to a local model is through `opencode`, and that is itself unverified.
  - `opencode models` lists **reachable** models, not a catalogue, and is silent about providers
    that are merely unconfigured.
  - `opencode` on the development machine fails **every** command because
    `~/.config/opencode/config.json` is invalid. A preflight that reports "no models" here would be
    wrong in a way the operator cannot act on; it must report the error verbatim.
- **The skill's phase 2 already has the shape this extends.** `skills/autopilot-deliver/SKILL.md`
  detects the project type, proposes `verify`, and the operator confirms.
- **`config.json` is tracked**, so an uncommitted edit is reverted by the first rejection path
  (`observed-behaviour.md`). The skill already blocks on an uncommitted `config.json`; the local
  bindings file is gitignored and does not share that hazard.

## Approach

Five tasks. The subcommand first, because the skill has nothing to read until it exists.

**Files created** — `runner/src/preflight.rs`, `tests/conformance/preflight.sh`

**Files modified** — `runner/src/main.rs`, `skills/autopilot-deliver/SKILL.md`,
`runner/install-project.sh` → the `install` subcommand's gitignore entry, `docs/guides/install.md`

---

## Work breakdown

### Task 1: `autopilot preflight` — report what this machine can reach

- **Done when:** the subcommand prints one JSON document describing every adapter, and an adapter whose CLI errors reports the error verbatim rather than an empty model list.
- **Verify:** `autopilot preflight | jq -e '.harnesses | length >= 3'` → true; with a stubbed failing `opencode`, `autopilot preflight | jq -r '.harnesses[] | select(.name=="opencode") | .error'` → the stub's stderr
- **Intent:** `docs/design/2026-08-16-multi-harness-role-pipeline-design.md`
- **Depends on:** —
- **Tier:** standard
- **Needs human:** no

**Files:**
- Create: `runner/src/preflight.rs`
- Modify: `runner/src/main.rs`

**Interfaces:**
- Produces: `autopilot preflight [--project <path>]` writing to stdout:

```json
{
  "harnesses": [
    {"name":"claude","available":true,"models":["sonnet","opus"],"error":null,"proven":true},
    {"name":"opencode","available":false,"models":[],"error":"Configuration is invalid at …","proven":false},
    {"name":"pi","available":true,"models":["deepseek/deepseek-v4-flash"],"error":null,"proven":false}
  ],
  "tiers_declared": ["light","standard","deep"],
  "bindings_present": ["standard","deep"],
  "unresolved": ["light"]
}
```

`unresolved` is the field the skill and the operator act on. `proven` comes from the adapter and is
false for any harness that has never executed against its real CLI.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn an_adapter_whose_cli_errors_reports_the_error_not_an_empty_list() {
    // opencode on the development machine fails every command on an invalid
    // user config. "no models" would be true and useless; the operator needs
    // the message to know it is fixable in thirty seconds.
    let h = stub_harness_that_errors("Configuration is invalid at /…/config.json");
    let r = preflight::describe(&h);
    assert_eq!(r.models.len(), 0);
    assert_eq!(r.error.as_deref(), Some("Configuration is invalid at /…/config.json"));
    assert!(!r.available);
}

#[test]
fn a_declared_tier_with_no_binding_appears_in_unresolved() {
    let out = preflight::run(&cfg_with_tiers(&["light","deep"]), &bindings(&["deep"]));
    assert_eq!(out.unresolved, vec!["light"]);
}

#[test]
fn a_binding_naming_a_model_the_harness_cannot_reach_is_unresolved() {
    // The half that is easy to forget: the harness is available, the tier is
    // bound, and the model simply is not there.
    let out = preflight::run(&cfg_with_tiers(&["light"]),
                             &binding("light", "pi", "anthropic/claude-opus"));
    assert_eq!(out.unresolved, vec!["light"]);
}
```

- [ ] **Step 2: Run and confirm failure.** `cd runner && cargo test preflight` → does not compile.

- [ ] **Step 3: Implement** — iterate the adapter registry, call `available()` and `models()`, and
  never let one adapter's failure abort the report. An operator with three harnesses and one broken
  config must still see the other two.

- [ ] **Step 4: Run tests and gates.**

```sh
cd runner && cargo test preflight && cargo clippy --all-targets -- -D warnings
```

- [ ] **Step 5: Commit**

```sh
git add runner/src/preflight.rs runner/src/main.rs
git commit -m "feat: report what harnesses and models this machine can reach

One detector, in the adapters, read by both the skill and guard_tiers. Two
detectors in two languages would disagree, and the one that matters is the one
the runner uses at 3 a.m.

An adapter whose CLI errors reports the error rather than an empty model list.
On the development machine opencode fails every command on an invalid user
config: 'no models' is true and useless, while the message is fixable in
thirty seconds."
```

---

### Task 2: `guard_tiers` and preflight share one code path

- **Done when:** `guard_tiers` calls the same `available()` the preflight reports, and a test proves a tier that preflight calls unresolved also stands the runner down.
- **Verify:** `cargo test tiers_agree` → 3 passed
- **Intent:** `docs/design/2026-08-16-multi-harness-role-pipeline-design.md`
- **Depends on:** Task 1
- **Tier:** standard
- **Needs human:** no

- [ ] **Step 1: The test that makes the agreement mechanical**

```rust
#[test]
fn what_preflight_calls_unresolved_stands_the_runner_down() {
    // Divergence here is the worst available outcome: the skill says the ladder
    // is fine and the runner disagrees every night, silently, at 3 a.m.
    let (cfg, bindings) = ladder_with_missing_harness();
    assert_eq!(preflight::run(&cfg, &bindings).unresolved, vec!["light"]);
    assert!(matches!(guard::tiers(&cfg, &bindings), GuardOutcome::StandDown(_)));
}
```

- [ ] **Step 2: Refactor so both call one function** rather than two that happen to agree today.

- [ ] **Step 3–4: Run, commit.**

---

### Task 3: `autopilot-deliver` phase 2 establishes the ladder

- **Done when:** phase 2 runs preflight, proposes a ladder, takes the operator's confirmation, writes both config layers, re-checks, and stops naming what is missing when anything is unresolved.
- **Verify:** `grep -c 'autopilot preflight' skills/autopilot-deliver/SKILL.md` → at least 1; `grep -c 'tiers.local.json' skills/autopilot-deliver/SKILL.md` → at least 2
- **Intent:** `docs/design/2026-08-16-multi-harness-role-pipeline-design.md` `skills/autopilot-deliver/SKILL.md`
- **Depends on:** Task 1
- **Tier:** standard
- **Needs human:** no

**Files:**
- Modify: `skills/autopilot-deliver/SKILL.md`

- [ ] **Step 1: Replace phase 2 in the Flow block**

```
2  Prepare the project (each part skipped if already done)
     deploy the runner (deploy.sh aborts if the target is a git checkout)
     autopilot not installed here?  → autopilot install
     labels missing?                → gh label create
     verify still the template?     → detect project type, propose, operator confirms
     TIER LADDER not established?   → autopilot preflight → propose → confirm → write
     any tier unresolved?           → STOP, naming the harness or model that is missing
     config.json uncommitted?       → block; git reset --hard would discard it
```

- [ ] **Step 2: Add the ladder section to the skill**

```markdown
## Establishing the tier ladder

Run `autopilot preflight` and read its JSON. **Do not run the CLIs yourself** — the adapters are the
one detector, and a second one written here would disagree with the one the runner uses at 3 a.m.

Propose a ladder from what came back, ordered cheapest first, and say what each entry costs:

    light     opencode  <a free or local model>        $0
    standard  claude    sonnet                         ~$5
    deep      claude    opus                           ~$15

Three rules when proposing:

- **Never propose a harness whose `proven` is false without saying so.** An adapter that has never
  run against its real CLI is a guess with a type signature.
- **Never invent a model name.** Propose only from the `models` list preflight returned.
- **The reviewer's tier is one step up the ladder**, so a two-entry ladder means the top tier
  reviews itself. Say that out loud; it is a real weakening and the operator should choose it
  knowingly rather than discover it.

The operator edits and confirms. Then write:

- `.autopilot/config.json` → `tiers`, `roles`, `pipeline`   — **committed**
- `.autopilot/tiers.local.json` → the bindings              — **gitignored**

Re-run `autopilot preflight` afterwards. If `unresolved` is non-empty, **stop** and name each entry
with the harness or model that is missing and what would fix it. There is no override: a ladder that
does not resolve is a loop that stands down every night, and that looks exactly like a quiet queue.
```

- [ ] **Step 3: Update the *Prepare steps* checklist** to include the ladder and the gitignore entry.

- [ ] **Step 4: Verify and commit.**

---

### Task 4: Phase 3 writes `tier:` labels from the plan

- **Done when:** sharding reads each task's **Tier** field, applies `tier:<name>`, refuses a tier the ladder does not declare, and no longer writes `model:` or `effort:`.
- **Verify:** `grep -c 'model:' skills/autopilot-deliver/SKILL.md` → 0; `sh tests/conformance.sh preflight` → IDENTICAL where comparable
- **Intent:** `docs/design/2026-08-16-multi-harness-role-pipeline-design.md` `skills/autopilot-deliver/SKILL.md`
- **Depends on:** Task 3
- **Tier:** standard
- **Needs human:** no

- [ ] **Step 1: Replace the tier paragraph in *Issue format***

State that a `Tier` naming something outside `config.tiers` is a **plan error, not a label to
create**. Creating `tier:medium` because a plan said so produces an issue the runner picks up and
then cannot resolve — a failure moved from planning time to 3 a.m., which is the wrong direction.

- [ ] **Step 2: Add the label-creation list** — `tier:<name>` for each declared tier, created by
  `autopilot install` idempotently, with the measured `already exists` rule from plan 2 task 20.

- [ ] **Step 3: Verify and commit.**

---

### Task 5: Exercise the preflight against this machine as it actually is

- **Done when:** the flow has been run against a throwaway repository on the development machine, where `opencode` is broken and only `claude` fully works, and the outcome is a resolved two-entry ladder or an honest stop.
- **Verify:** in the throwaway repository, `autopilot preflight | jq -r '.unresolved[]'` prints nothing, and `.autopilot/tiers.local.json` names only harnesses whose `available` was true
- **Intent:** `docs/design/2026-08-16-multi-harness-role-pipeline-design.md` `docs/reference/observed-behaviour.md`
- **Depends on:** Task 4
- **Tier:** deep
- **Needs human:** yes

`Needs human: yes` — the value of this task is that the machine is in a **realistically broken**
state, and only a person can judge whether what the preflight said about it was useful.

- [ ] **Step 1: Run it as-is, before fixing anything**

The development machine has a broken `opencode` config, `pi` with one provider, LM Studio off, and
no `ollama` or `codex`. That is not an obstacle to the test — **it is the test**. A preflight that is
only ever run on a healthy machine has never been run.

- [ ] **Step 2: Judge the output as an operator, not as its author**

Does it say enough to act on? Does it distinguish *not installed* from *installed and misconfigured*
from *installed, configured, and out of credentials*? Those need three different fixes and the
message must not blur them.

- [ ] **Step 3: Then fix the machine and run it again**

Repair the `opencode` config, start the LM Studio server, and confirm the ladder that was unresolved
now resolves — and that a **local model tier**, which no measurement has yet confirmed is reachable
at all, either works or is recorded as still unproven.

- [ ] **Step 4: Record both runs in `docs/reference/observed-behaviour.md`**, with the date and the
  verbatim output. The broken run is the more valuable of the two.

- [ ] **Step 5: Move anything still open into `docs/product/open-items.md`** — in particular whether
  `opencode` reaches `ollama` and `lmstudio` in practice, which is currently an open question and the
  only route to a local tier.

---

## Verification

| Check | Command | Passing looks like |
|---|---|---|
| Unit | `cd runner && cargo test preflight tiers_agree` | all green |
| Gates | `cargo clippy --all-targets -- -D warnings && cargo fmt --check` | no output |
| One detector | Task 2's test | preflight and `guard_tiers` cannot disagree |
| Skill updated | `grep -c 'model:' skills/autopilot-deliver/SKILL.md` | `0` |
| Exercised for real | Task 5 | run on a machine that is genuinely part-broken, twice |

## Documentation to update

- [ ] `skills/autopilot-deliver/SKILL.md` — phases 2 and 3, the ladder section, prepare steps (Tasks 3, 4)
- [ ] `docs/guides/install.md` — the ladder, the two config layers, the proven-adapter table (Task 3)
- [ ] `docs/reference/observed-behaviour.md` — both preflight runs, verbatim (Task 5)
- [ ] `docs/product/open-items.md` — whether `opencode` reaches local runtimes (Task 5)
- [ ] `docs/README.md` status table — this plan's row (Task 5)

## Risks

| Risk | Level | Mitigation |
|---|---|---|
| The skill and `guard_tiers` disagree, so the ladder passes preflight and stands down nightly | **High** | Task 2 makes them one code path and asserts it. This is the failure this repository has recorded five times and it must not have a sixth form |
| A two-entry ladder means the top tier reviews its own tier's work | Medium | The skill says so out loud when proposing. It is a real weakening, and the operator chooses it knowingly rather than discovering it |
| The operator confirms a ladder naming an unproven adapter | Medium | `proven: false` is carried through preflight to the proposal, and the skill must say it |
| A local model tier is proposed that no measurement supports | Medium | Task 5 either proves it or records it as still unproven; until then no template proposes one |
| Preflight is slow enough that the skill feels stuck | Low | It runs `--version`-class commands per harness; if any is slow, cache within one skill session, never across |

## Out of scope

- **Fixing the operator's machine.** The preflight reports; repairing an `opencode` config or
  starting an LM Studio server is the operator's action, and Task 5 does it as a person, not as code.
- **Installing harnesses.** Detecting them is enough. A skill that installs CLIs is a skill that
  changes the machine outside the project root.
- **`codex` and local-model adapters.** Plan 2 out-of-scope, unchanged here.
- **Re-establishing the ladder from inside a scheduled run.** `guard_tiers` reports and stands down.
  Rebinding a tier is a decision, and §2.4 reserves decisions for a person.
