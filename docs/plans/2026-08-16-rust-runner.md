# The Rust runner: harnesses, tiers, and the role pipeline — implementation plan

> **Type:** Plan · **Date:** 2026-08-16 · **Status:** Awaiting approval · **Rewritten 2026-08-19 against decisions 26–42**
> **Prerequisite:** docs/plans/2026-08-16-planning-skill.md
> **Scope:** The runner half of the multi-harness design, taken as one cutover: a Rust binary that
> drives one of several agent CLIs at a model the project chooses per tier, through an implement →
> test → review pipeline whose every step is journalled to one append-only file.

> **For agentic workers:** Steps use checkbox (`- [ ]`) syntax for tracking. Implement one task at a
> time; each task's `Verify:` names the command that must pass before it is committed.

**Goal:** Replace the POSIX shell runner with a single Rust binary, `autopilot`, whose subcommands
are `run-once`, `status`, `start`, `stop`, and `install`. `git` and `gh` remain child processes. A
`Harness` trait with one file per CLI is the only place a harness name appears. The shell runner is
frozen as the reference specification and is deleted only after a real three-role run (the final
task, which needs a human).

**Architecture:** per `docs/design/2026-08-16-multi-harness-role-pipeline-design.md`. One binary;
`runner/src/` holds the modules; one file per harness under `runner/src/harness/`.

**Tech Stack:** Rust 2021, four crates only — `serde`, `serde_json`, `time` (with `local-offset`),
`libc`. No async runtime: the runner is sequential by ADR-0001 and `std::process::Command` is
sufficient.

**Spec:** [`docs/design/2026-08-16-multi-harness-role-pipeline-design.md`](../design/2026-08-16-multi-harness-role-pipeline-design.md)
— decisions 1–16 and 24–42, and the sections *Why Rust*, *The harness adapter contract*, *The tier
ladder*, *Why three roles at all*, *The role pipeline*, *Verdicts travel through the file system*,
*The branch model*, and *The journal*.

## Global Constraints

Copied from the spec. Every task implicitly includes these.

- **Four crates, and no more**, without an ADR: `serde`, `serde_json`, `time`, `libc`.
- **No module exceeds ~200 lines.**
- **No harness name appears outside `src/harness/`.** Grep-checkable, and a test.
- **The runner never references a skill or the dashboard.** Grep-checkable, and a test.
- `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` clean before every commit.
- **No `unwrap()` or `expect()` on any path a scheduled run can reach.** Tests only.
- Tests run against a **real throwaway git repository**. `gh` and every agent CLI are stubbed with
  fixture programs on `PATH`, and every fixture's shape is **recorded, never invented**.
- The four invariants of `CLAUDE.md` §2 plus the four added by the design each carry at least one
  test that fails if the invariant is removed.
- The external contract does not change — `.autopilot/config.json`, `.autopilot/state.json`,
  `.autopilot/logs/`, the launchd plist, and every `gh` label name — except where the design says
  otherwise (the tier ladder, the journal, and `tier:` labels replace `model:`/`effort:`).

## Context

The runner works and does not vary: every task is implemented, tested, and reviewed by one
invocation of one hard-coded CLI. The design argues at length why that must become a per-task
choice of model through a harness abstraction, and why the language changes at the same time: of
the defects that survived 166 passing shell tests, most were shell semantics, and three of the four
classes are unrepresentable in Rust.

Decision 26 had sequenced the work so the Rust cutover carried one harness and one role, with the
tier ladder, the remaining adapters, the preflight, and the dashboard waiting behind a cost
measurement. **Decision 42, the operator's explicit override dated 2026-08-19, sets that sequencing
aside for this build**: all four plans are implemented together, with no second-harness measurement
preceding the build. The risk decision 26 was written to avoid — building the ladder and the
remaining adapters from a sample of one measured harness — is accepted knowingly. Nothing in this
plan reopens decision 26 for future adapter work; it authorises this one build.

## What we already know

- **The shell suite passes** (`sh tests/run.sh` reports `ALL PASS`) and the shell runner stays as
  the frozen reference until the final needs-human task deletes it.
- **Seven defects and their rules are recorded** in `docs/reference/observed-behaviour.md`. Every one
  is a finding about `gh`, `git`, or the agent CLI — which is why those stay child processes, and
  why each rule needs a Rust test rather than a fresh discovery.
- **Measured harness behaviour.** `claude` reports `total_cost_usd` on its final `result` event
  (recorded fixture `tests/fixtures/claude-result-success.json`, 2026-08-16). On 2026-08-19 the `pi`
  CLI was measured directly and the transcript recorded under `tests/fixtures/pi-run-*.jsonl`:
  `pi -p --mode json` streams `message_start`/`message_end`/`turn_end`/`agent_end` events, carries
  cost on `usage.cost.total`, and **exits 0 even when the model errors** — the failure is
  `"stopReason":"error"` plus `"errorMessage"` inside the JSON, not the exit code. `pi --list-models`
  lists reachable models (recorded). `opencode models` on this machine fails on an invalid user
  config (`~/.config/opencode/config.json`), exits 1, and puts the error on stderr; `opencode
  providers list` exits 0 and lists credentials.
- **On this machine**: `claude` and `pi` (deepseek) are usable; `opencode` is installed but its run
  path is unproven; `codex` is not installed. An adapter that has never produced a real run
  transcript ships labelled **unproven**.
- **Five configuration keys are declared and read by nothing** in the shell runner:
  `agent.turn_timeout_s`, `pacing.max_attempts_per_issue`, `autonomy.default`, a cumulative spend
  ceiling, and the `launchctl` runs/last-exit reporting in `ctl.sh status`. This plan makes the
  first four real and the fifth a subcommand.

## Approach

Seventeen tasks in five groups. Foundations first (everything writes to the journal and reads the
config), then the pieces with a shell predecessor to port, then the genuinely new pipeline, then
the operator surface, then the needs-human cutover.

| Group | Tasks | What it establishes |
|---|---|---|
| Foundations | 1–3 | Cargo layout, gates, the single spawn door, logging, the journal |
| Ported from shell | 4–8 | config, tiers, state, label/gh/queue, intent, guards, verify |
| The harness abstraction | 9–10 | The trait, the shared classifier, the three adapters |
| Genuinely new | 11–14 | Verdicts and prompts, settlement with task branches, the role pipeline, timeouts and the run-once flow |
| Operator surface | 15–16 | `status`/`start`/`stop`, `install` |
| Cutover | 17 | A real three-role run, then deletion of the shell runner — **needs human** |

**Files created** — `runner/Cargo.toml`, `runner/src/*.rs` per the design's module list,
`runner/templates/prompt-{implement,test,review}.tmpl`, `runner/tests/*.rs`.

**Files modified** — `.gitignore` (add `runner/target/`), `runner/templates/config.json`,
`runner/VERSION`, `docs/reference/observed-behaviour.md`, `docs/guides/install.md`.

**Files deleted at task 17, not before** — `runner/run-once.sh`, `runner/lib/`, `runner/ctl.sh`,
`runner/install-project.sh`, `runner/deploy.sh`, `runner/templates/prompt.tmpl`, and the
shell-level tests that source `lib/*.sh` directly.

---

## Work breakdown

### Task 1: Cargo scaffold, the gates, and the boundary tests

- **Done when:** `cargo build` produces `autopilot`, `cargo fmt --check` and
  `cargo clippy --all-targets -- -D warnings` are clean, and the boundary tests exist and pass.
- **Verify:** `cd runner && cargo test --test boundaries` → 3 passed; `cargo clippy --all-targets -- -D warnings` → no warnings
- **Intent:** docs/design/2026-08-16-multi-harness-role-pipeline-design.md CLAUDE.md
- **Depends on:** —
- **Tier:** light
- **Needs human:** no

**Files:**
- Create: `runner/Cargo.toml`, `runner/src/main.rs`, `runner/tests/boundaries.rs`
- Modify: `.gitignore`

- [ ] **Step 1: Write `runner/Cargo.toml`** with the four crates only and `panic = "abort"` in release.
- [ ] **Step 2: Write `runner/tests/boundaries.rs`** with three tests: no harness name (`claude`,
  `opencode`, `codex`, or the whole word `pi`) outside `src/harness/`; no reference to `skills/` or
  `dashboard`; no module over 200 lines.
- [ ] **Step 3: Write `main.rs`** parsing `--project` for `run-once` and exiting with a usage error otherwise.
- [ ] **Step 4: Run the gates** and read the output.
- [ ] **Step 5: Commit.**

### Task 2: `spawn.rs`, `log.rs`, and `journal.rs`

- **Done when:** every subprocess goes through one timed spawn helper; log lines match the shell
  format; the journal appends one JSONL object per event to `.autopilot/journal.jsonl`, with a
  nullable `issue` and `cost_source`.
- **Verify:** `cd runner && cargo test journal spawn` → all green; a written line parses with `jq -e .` and carries `cost_source`
- **Intent:** docs/design/2026-08-16-multi-harness-role-pipeline-design.md
- **Depends on:** Task 1
- **Tier:** standard
- **Needs human:** no

**Files:**
- Create: `runner/src/spawn.rs`, `runner/src/log.rs`, `runner/src/journal.rs`

**Interfaces:**
- `spawn::run(cmd: &mut std::process::Command, stdin: &[u8], timeout_s: u64) -> io::Result<SpawnOutcome>`
  where `SpawnOutcome` is `Exited(Output)` or `TimedOut`. It runs the child in its own session
  (`setsid`) and, on timeout, sends `SIGTERM` then `SIGKILL` to the whole process group. **Every**
  `git`, `gh`, and harness invocation in the crate goes through it.
- `log::info/warn/error(&str)` — `TIMESTAMP LEVEL message` to stderr and, when the log file is set,
  appended there too.
- `journal::Journal::open(project, max_bytes)`, `journal::Journal::append(&mut self, Event)`,
  `journal::Event` with variants `WakeStart { issue: Option<u64>, tier: Option<String> }`,
  `RoleStart`, `RoleEnd`, `Verdict`, `Verify`, `WakeEnd`; `journal::CostSource { Reported, Unknown }`.

- [ ] **Step 1: Write the failing tests**, including: a `role_end` records `cost_source`; an unknown
  cost is serialised as `null`, never `0.0`; a `role_start` with no `role_end` is detectable; a
  timeout kills a forked child and its descendants; rotation rolls `journal.jsonl` to
  `journal-<date>.jsonl` at the configured size without deleting.
- [ ] **Step 2: Run and confirm they fail.**
- [ ] **Step 3: Implement** `spawn.rs`, `log.rs`, `journal.rs`. The journal is one file per project
  (decision 30), flushed on every event.
- [ ] **Step 4: Run tests, `cargo clippy`, `cargo fmt`.**
- [ ] **Step 5: Commit.**

### Task 3: `config.rs`, `tier.rs`, and `state.rs`

- **Done when:** `config.json` and `.autopilot/tiers.local.json` load together; every declared tier
  resolves to a binding or fails by name; a stored zero round-trips as zero; and the per-issue
  attempt counter has explicit storage and an eviction rule.
- **Verify:** `cd runner && cargo test config tier state` → all green, including one asserting an old config carrying `agent.default_model` fails naming the key, and one asserting the top tier offset by one resolves to itself
- **Intent:** docs/design/2026-08-16-multi-harness-role-pipeline-design.md
- **Depends on:** Task 2
- **Tier:** standard
- **Needs human:** no

**Files:**
- Create: `runner/src/config.rs`, `runner/src/tier.rs`, `runner/src/state.rs`
- Modify: `runner/templates/config.json`

**Interfaces:**
- `config::Config::load(project) -> Result<Config, ConfigError>`; `ConfigError` distinguishes
  `NotFound`, `Invalid{path,msg}`, and `UnboundTier(String)`.
- `tier::resolve(cfg, name, offset) -> Result<(&str, &TierBinding), ConfigError>`; offset saturates.
- `state::State::init(path)`, `get_num`, `get_str`, `set_num`, `set_str`, `bump`, `save`; plus
  `attempt_count(issue)`, `record_attempt(issue)`, `clear_attempt(issue)`, and `prune_attempts(open)`.

- [ ] **Step 1: Write the failing tests.** The three that matter: a tier with no binding is refused
  by name; an old config with `agent.default_model` fails loudly naming what is missing; a stored
  `0.0` budget is not confused with absent. Plus state round-trip and attempt-eviction tests.
- [ ] **Step 2: Run and confirm failure.**
- [ ] **Step 3: Implement.** Layer 1 (`config.json`) carries the tier *names*, roles, pipeline,
  agent, verify, pacing, autonomy, and the new `journal.max_bytes`. Layer 2
  (`.autopilot/tiers.local.json`, gitignored) carries `tier -> {harness, model, effort, budget_usd}`.
  `agent.default_model`, `agent.default_effort`, and `agent.max_budget_usd` are gone and their
  presence is a load error. Update `runner/templates/config.json` to the new schema, including
  `queue.tier_label_prefix` and `agent.default_tier`.
- [ ] **Step 4: Run tests, gates.**
- [ ] **Step 5: Commit.**

### Task 4: `label.rs`, `gh.rs`, and `queue.rs`

- **Done when:** the launchd label derives from the `origin` remote; every `gh` call names the
  repository explicitly and captures gh's stderr; a missing ready label is distinguished from an
  empty queue; a failed claim stops the run with gh's message.
- **Verify:** `cd runner && cargo test queue label` → all green, with the four `observed-behaviour.md` rules as named tests
- **Intent:** docs/reference/observed-behaviour.md docs/decisions/0004-job-files-live-on-the-internal-disk.md
- **Depends on:** Task 3
- **Tier:** deep
- **Needs human:** no

**Files:**
- Create: `runner/src/label.rs`, `runner/src/gh.rs`, `runner/src/queue.rs`

**Interfaces:**
- `label::repo_slug_for_project(dir) -> Option<String>`, `label::label_for_project(dir) -> String`.
- `gh::run(args: &[&str], timeout_s: u64) -> Result<Output, GhError>` capturing stdout **and** stderr.
- `queue::Queue::init(project) -> Result<Queue, QueueError>`; `load`, `pick`, `field`, `has_label`,
  `claim`, `release`.

- [ ] **Step 1: Write the failing tests by porting every assertion from `tests/test_queue.sh` and
  `tests/test_label.sh`, plus the four named rules: every gh call names the repository; a missing
  ready label is not an empty queue; loading and picking are separate calls; a failed claim returns
  gh's stderr to the caller.
- [ ] **Step 2: Run and confirm failure.**
- [ ] **Step 3: Implement.** The slug parse is shared between `label.rs` and `queue.rs` (one parse,
  not two). The label fallback (no remote) uses a digest of the absolute path.
- [ ] **Step 4: Run tests, gates.**
- [ ] **Step 5: Commit.**

### Task 5: `intent.rs`

- **Done when:** an issue body's `Intent:` line yields repo-relative paths; an absolute path, a `..`
  component, a missing file, a directory, or a symlink escaping the root is refused by name; refusal
  happens before any claim.
- **Verify:** `cd runner && cargo test intent` → all green, ported from `tests/test_intent.sh` including the symlink-escape case
- **Intent:** docs/decisions/0005-intent-binding.md
- **Depends on:** Task 4
- **Tier:** standard
- **Needs human:** no

**Files:**
- Create: `runner/src/intent.rs`

**Interfaces:**
- `intent::resolve(body: &str, project: &Path, marker: &str) -> Result<Vec<PathBuf>, IntentError>`.

- [ ] **Step 1: Port every assertion from `tests/test_intent.sh`** as Rust tests, including the
  symlink-inside-root accepted case and the symlink-escape refused case.
- [ ] **Step 2: Run and confirm failure.**
- [ ] **Step 3: Implement** with `std::fs::canonicalize` after a textual `..`-component check, so the
  refusal names the actual escape.
- [ ] **Step 4: Run tests, gates.**
- [ ] **Step 5: Commit.**

### Task 6: `guard.rs` and `verify.rs`

- **Done when:** lock, main-branch, quiet hours, daily cap, `STOP`, `resume_after`, and the new
  `guard_tiers` all stand a run down with a clean exit; the lock is released on every path including
  panic; verify runs commands in order inside the project and refuses an empty list.
- **Verify:** `cd runner && cargo test guard verify` → all green
- **Intent:** docs/design/2026-08-16-multi-harness-role-pipeline-design.md
- **Depends on:** Task 3, Task 4
- **Tier:** standard
- **Needs human:** no

**Files:**
- Create: `runner/src/guard.rs`, `runner/src/verify.rs`

**Interfaces:**
- `guard::GuardOutcome { Proceed, StandDown(reason) }`; `guard::all(cfg, state, project)` and
  `guard::tiers(cfg, needed: &[&str])`; `guard::Lock` whose `Drop` releases.
- `verify::run(cfg, project) -> Result<(), VerifyFailure>`.

- [ ] **Step 1: Port `tests/test_guards.sh` and `tests/test_verify.sh`**, then add:
  `guard_tiers` checks **only** the tiers the picked task needs (decision 38) and runs **after** the
  queue is known; a lock is released even when the run panics (a `catch_unwind` test).
- [ ] **Step 2: Run and confirm failure.**
- [ ] **Step 3: Implement.** Quiet hours use local time (the `time` crate's `local-offset`) with the
  same boundary and midnight-wrapping semantics the shell tests pin.
- [ ] **Step 4: Run tests, gates.**
- [ ] **Step 5: Commit.**

### Task 7: `harness/mod.rs` and `harness/claude.rs`

- **Done when:** the `Harness` trait exists with the design's methods; the default classifier uses
  exit status plus `probe()`; the claude adapter builds the shell's argument list, sends the prompt
  on stdin, and keeps the two `observed-behaviour.md` refinements.
- **Verify:** `cd runner && cargo test harness` → all green for the default classifier and `claude`, using the recorded claude fixture
- **Intent:** docs/reference/observed-behaviour.md docs/design/2026-08-16-multi-harness-role-pipeline-design.md
- **Depends on:** Task 2
- **Tier:** deep
- **Needs human:** no

**Files:**
- Create: `runner/src/harness/mod.rs`, `runner/src/harness/claude.rs`

**Interfaces:**
- `harness::Classification { Ok, TaskFailure, ProviderUnavailable }`;
  `harness::RunParams { model, effort, budget_usd, timeout_s, permission_mode }`;
  `harness::RunOutcome { status, log, cost_usd, cost_source, duration_s }`;
  `harness::Harness` (trait), `harness::by_name(&str) -> Option<Box<dyn Harness>>`,
  `harness::registry() -> Vec<Box<dyn Harness>>`.

- [ ] **Step 1: Write the failing tests** against a fake harness and the recorded claude fixture:
  nonzero exit with a live provider is a task failure; nonzero exit with a dead provider is
  `ProviderUnavailable` (invariant 2); only the final `result` event decides; `is_error:false` is not
  treated as absent; a throttled `rate_limit_event` is `ProviderUnavailable`; cost comes from the
  final event and is marked `reported`.
- [ ] **Step 2: Run and confirm failure.**
- [ ] **Step 3: Implement.** The default `classify` lives in `harness/mod.rs`; only `claude.rs`
  overrides it. The spawn door is `spawn::run`.
- [ ] **Step 4: Run tests, gates.**
- [ ] **Step 5: Commit.**

### Task 8: `harness/pi.rs` and `harness/opencode.rs`

- **Done when:** both adapters exist; `pi`'s `available()` and `classify` read the JSON, not the exit
  code; `pi` ships proven (transcript recorded 2026-08-19); `opencode` ships unproven and reports its
  config error verbatim.
- **Verify:** `cd runner && cargo test harness` → all green, including one asserting a `pi` stream with `stopReason:"error"` and exit 0 classifies as a task failure, and one asserting `opencode` reports its stderr rather than an empty model list
- **Intent:** docs/design/2026-08-16-multi-harness-role-pipeline-design.md docs/reference/observed-behaviour.md
- **Depends on:** Task 7
- **Tier:** standard
- **Needs human:** no

**Files:**
- Create: `runner/src/harness/pi.rs`, `runner/src/harness/opencode.rs`
- Modify: `tests/fixtures/` (add the recorded `pi-run-success.jsonl`, `pi-run-error.jsonl`, `pi-list-models.txt`)

**Interfaces:**
- `pi`'s `check(model)` runs `pi auth check --provider <provider> --json` and reads `status`, and
  `list_models()` runs `pi --list-models`. `pi`'s `classify` reads `stopReason`/`errorMessage` from
  the final `turn_end`/`agent_end` events because the process exits 0 on a model error.
- `opencode`'s `check(model)` runs `opencode providers list`; `list_models()` runs `opencode models`
  and surfaces stderr as the error.

- [ ] **Step 1: Write the failing tests** against the recorded `pi` fixtures and a stubbed opencode
  that prints a config error to stderr and exits 1.
- [ ] **Step 2: Run and confirm failure.**
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run tests, gates.**
- [ ] **Step 5: Commit.**

### Task 9: Prompt templates and `verdict.rs`

- **Done when:** three role templates exist; each role writes a verdict to
  `.autopilot/runs/<issue>/<wake-id>/attempt-<a>/round-<k>/{tester,reviewer}-verdict.json`; a missing
  or malformed verdict is a `TaskFailure`, never a pass; a verdict that existed before its role was
  invoked is refused.
- **Verify:** `cd runner && cargo test verdict` → all green, including invariant 5 and the forgery check
- **Intent:** docs/design/2026-08-16-multi-harness-role-pipeline-design.md
- **Depends on:** Task 7
- **Tier:** standard
- **Needs human:** no

**Files:**
- Create: `runner/templates/prompt-implement.tmpl`, `runner/templates/prompt-test.tmpl`,
  `runner/templates/prompt-review.tmpl`, `runner/src/verdict.rs`
- Delete: `runner/templates/prompt.tmpl` (superseded — deferred to task 17 with the other shell artifacts)

**Interfaces:**
- `verdict::Verdict { verdict, reason, evidence }`; `verdict::read(path) -> Result<Verdict, VerdictError>`
  (`Missing` or `Malformed`); `verdict::prepare_dir(project, issue, wake, attempt, round)` creates the
  directory immediately before invoking a role and refuses if a verdict file already exists there.

- [ ] **Step 1: Write the failing tests**: a missing verdict is never a pass; a malformed verdict is
  never a pass; a verdict that already existed before the role was invoked is refused (decision 35).
- [ ] **Step 2: Run and confirm failure.**
- [ ] **Step 3: Write the three templates.** Each keeps the existing prompt's prohibitions. The test
  template instructs: *you did not write this code; test the task as the plan states it*. The review
  template takes a lens name and is given only that lens's question. All three keep a byte-identical
  prefix and name paths rather than pasting excerpts.
- [ ] **Step 4: Implement `verdict.rs`.**
- [ ] **Step 5: Run tests, gates.**
- [ ] **Step 6: Commit.**

### Task 10: `settle.rs` and the branch model

- **Done when:** work happens on `autopilot/task-<n>`; a green verify rebases onto `autopilot/main`,
  verifies again, and `merge --ff-only`s; a pause commits a `WIP:` commit to the task branch, pushes,
  and labels `blocked`; every rejection path rewinds to `START_SHA`.
- **Verify:** `cd runner && cargo test settle` → all green
- **Intent:** docs/design/2026-08-16-multi-harness-role-pipeline-design.md docs/reference/observed-behaviour.md
- **Depends on:** Task 4, Task 6
- **Tier:** deep
- **Needs human:** no

**Files:**
- Create: `runner/src/settle.rs`

**Interfaces:**
- `settle::guard_branch(project, issue) -> Result<(), SettleError>` (exactly this issue's task branch);
  `settle::reset(project, start_sha)`; `settle::merge`, `settle::pause`, `settle::fail`,
  `settle::block`.

- [ ] **Step 1: Port `tests/test_settle.sh`**, then add the branch-model tests: the agent may not work
  on `autopilot/main` (invariant 6); a pause keeps the work and a failure discards it (decision 34);
  every rejection path rewinds to the recorded SHA, not `HEAD` (`observed-behaviour.md` §7); the
  push rule (push whatever `HEAD` is, always) still holds.
- [ ] **Step 2: Run and confirm failure.**
- [ ] **Step 3: Implement.** `pause` commits `WIP:` work to the task branch and pushes it — invariant 1
  is *no unverified work reaches `autopilot/main`*, not *nothing is committed*.
- [ ] **Step 4: Run tests, gates.**
- [ ] **Step 5: Commit.**

### Task 11: `pipeline.rs` — the three roles

- **Done when:** the implement → verify → test → review loop runs with `max_rounds`; review lenses
  each get a fresh call and always run; a red verify returns to the implementer; every exit path is
  journalled; the wake budget is checked before each role, never mid-role.
- **Verify:** `cd runner && cargo test pipeline` → all green, one test per exit path plus the decision-6 and decision-24 cases
- **Intent:** docs/design/2026-08-16-multi-harness-role-pipeline-design.md
- **Depends on:** Task 9, Task 10, Task 6
- **Tier:** deep
- **Needs human:** no

**Files:**
- Create: `runner/src/pipeline.rs`

**Interfaces:**
- `pipeline::run(cfg, project, issue, tier, journal) -> Outcome` where `Outcome` is
  `Merged | Failed | Paused | StoodDown`.

- [ ] **Step 1: Write one test per exit path** from the design's list — guard stand-down, unavailable
  provider, intent refusal, failed claim, implement failure, rounds exhausted, attempts exhausted, a
  lens rejecting, red final verify, success — plus: a reviewer that repaired does not gate its own
  repair (decision 6 retired by decision 28, so the final lenses still run); each review lens is a
  separate invocation (decision 24); attempts exhausted pauses rather than failing again.
- [ ] **Step 2: Run and confirm failure.**
- [ ] **Step 3: Implement** the loop exactly as the design's pseudocode reads: `verify()` at every
  boundary; red verify → implementer; the tester writes tests from a clean context; the reviewer
  adjudicates and repairs; final lenses always run; rebase before final verify; conflict pauses.
- [ ] **Step 4: Run tests, gates.**
- [ ] **Step 5: Commit.**

### Task 12: `main.rs` — the run-once flow and wake budgeting

- **Done when:** `autopilot run-once --project <path>` performs the full flow; `turn_timeout_s` kills
  a hung child and its descendants; `wake_timeout_s` ends the wake; neither leaves the lock held.
- **Verify:** `cd runner && cargo test --test run_once` → all green against a real throwaway git repository with stubbed `gh` and harness CLIs
- **Intent:** docs/design/2026-08-16-multi-harness-role-pipeline-design.md
- **Depends on:** Task 11
- **Tier:** deep
- **Needs human:** no

**Files:**
- Create: `runner/src/main.rs` (the run-once flow), `runner/tests/run_once.rs`

**Interfaces:**
- The binary exits 0 for a normal stand-down and non-zero only for a configuration or claim failure.

- [ ] **Step 1: Write the integration tests** against a real throwaway repo with a bare local remote:
  STOP short-circuits; empty queue releases the lock; a red verify discards work; a full three-role
  run merges and closes; usage exhaustion sets `resume_after` and consumes no retry; the wake budget
  is checked before each role.
- [ ] **Step 2: Run and confirm failure.**
- [ ] **Step 3: Wire `main.rs`**: parse subcommands, load config and state, run the guards, pick and
  claim an issue, resolve intent and tier, run the pipeline, settle the circuit breaker. The lock is
  a `Drop` guard.
- [ ] **Step 4: Run tests, gates.**
- [ ] **Step 5: Commit.**

### Task 13: `autopilot status | start | stop`

- **Done when:** `status` reports `launchctl` runs and last exit code, compares the deployed `VERSION`
  against the plugin's, and names orphaned roles from the journal; `start` and `stop` verify with
  `launchctl print` rather than assuming.
- **Verify:** `cd runner && cargo test ctl` → all green
- **Intent:** docs/reference/observed-behaviour.md docs/decisions/0002-off-until-explicitly-started.md
- **Depends on:** Task 2
- **Tier:** standard
- **Needs human:** no

**Files:**
- Create: `runner/src/ctl.rs`

**Interfaces:**
- `ctl::status/start/stop(project)` shelling out to `launchctl` through `spawn::run`.

- [ ] **Step 1: Port `tests/test_ctl.sh`** and add the three gaps: status reports a loaded job that
  has never run; status reports a stale deployed runner; status names a role that started and never
  ended.
- [ ] **Step 2: Run and confirm failure.**
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run tests, gates.**
- [ ] **Step 5: Commit.**

### Task 14: `autopilot install`

- **Done when:** it writes `config.json`, the gitignore entries (including `tiers.local.json`), and
  the plist; creates the labels idempotently; and refuses before writing anything when `origin` is
  missing.
- **Verify:** `cd runner && cargo test install` → all green
- **Intent:** docs/reference/observed-behaviour.md docs/decisions/0004-job-files-live-on-the-internal-disk.md
- **Depends on:** Task 4
- **Tier:** standard
- **Needs human:** no

**Files:**
- Create: `runner/src/install.rs`

**Interfaces:**
- `install::run(project, interval)` — writes config, gitignore, plist, and the fixed labels plus the
  configured ready label, using the measured `already exists` rule from `observed-behaviour.md`.

- [ ] **Step 1: Port `tests/test_install.sh`** plus the `gh label create` rule (only stderr containing
  `already exists` is benign) and the no-origin refusal.
- [ ] **Step 2: Run and confirm failure.**
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run tests, gates.**
- [ ] **Step 5: Commit.**

### Task 15: The preflight subcommand (shared with plan 2)

- **Done when:** `autopilot preflight` prints one JSON document describing every adapter, and an
  adapter whose CLI errors reports the error verbatim rather than an empty model list.
- **Verify:** `cd runner && cargo test preflight` → all green (implemented in full under
  docs/plans/2026-08-16-deliver-preflight.md)
- **Intent:** docs/design/2026-08-16-multi-harness-role-pipeline-design.md
- **Depends on:** Task 8
- **Tier:** standard
- **Needs human:** no

**Files:**
- Create: `runner/src/preflight.rs` (implementation owned by plan 2)

**Interfaces:**
- `preflight::run(project) -> PreflightReport`; the subcommand is registered here so the binary
  surface is complete, and the body lives with plan 2.

- [ ] **Step 1: Register the `preflight` subcommand** and delegate to `preflight.rs`.
- [ ] **Step 2: Run `cargo build` and confirm the subcommand is reachable.**
- [ ] **Step 3: Commit.**

### Task 16: Documentation and the version bump

- **Done when:** `runner/VERSION` reads `0.3.0`; `docs/guides/install.md` gains the tier ladder, the
  two config layers, and the proven-adapter table; `docs/reference/observed-behaviour.md` gains the
  2026-08-19 measurements.
- **Verify:** `grep -c 'tiers.local.json' docs/guides/install.md` → at least 1; `grep -c '2026-08-19' docs/reference/observed-behaviour.md` → at least 1
- **Intent:** docs/reference/observed-behaviour.md docs/guides/install.md
- **Depends on:** Task 14
- **Tier:** light
- **Needs human:** no

**Files:**
- Modify: `runner/VERSION`, `docs/guides/install.md`, `docs/reference/observed-behaviour.md`

- [ ] **Step 1: Update the docs and the version.**
- [ ] **Step 2: Commit.**

### Task 17: The cutover — a real run, then deletion

- **Done when:** one issue with a valid `Intent:` and `tier:` label runs implement → test → review
  against a real repository with a real harness; verification passes; the task branch fast-forwards
  into `autopilot/main`; the issue closes; `.autopilot/journal.jsonl` shows a matching `role_end` for
  every `role_start`; and only then are the shell runner and its tests deleted.
- **Verify:** in a throwaway repository, the full run above is observed and the journal is read back; then `git rm` the shell files listed in *Files deleted*.
- **Intent:** docs/design/2026-08-16-multi-harness-role-pipeline-design.md docs/reference/observed-behaviour.md
- **Depends on:** Task 16
- **Tier:** deep
- **Needs human:** yes

`Needs human: yes` — this is the Definition-of-Done clause that asks for a real run, and it is the
clause this repository has skipped before. **Not performed in this unattended run**: the binary, its
tests, and the preflight are delivered; the real three-role run, the deletion of the shell runner,
and the switch of `deploy.sh`/the plist to the binary are left for a human. The shell runner remains
the frozen reference and its suite stays green.

- [ ] **Step 1: Exercise it for real** before deleting anything.
- [ ] **Step 2: Exercise the failure paths for real too** (red verify, reviewer repair, rounds-exhausted pause).
- [ ] **Step 3: Only now, delete the shell runner** and switch `deploy.sh` and the plist template to the binary.
- [ ] **Step 4: Record what the real run taught** in `docs/reference/observed-behaviour.md`.
- [ ] **Step 5: Commit the cutover as one commit.**

---

## Verification

| Check | Command | Passing looks like |
|---|---|---|
| Unit and integration | `cd runner && cargo test` | all green |
| Lint and format | `cargo clippy --all-targets -- -D warnings && cargo fmt --check` | no output |
| Boundaries | `cargo test --test boundaries` | 3 passed |
| No panics on reachable paths | `grep -rn 'unwrap()\|expect(' runner/src` | only inside `#[cfg(test)]` |
| Shell reference still green | `sh tests/run.sh` | `ALL PASS` (until task 17) |
| Exercised for real | Task 17 | three roles, a green merge, a journal with no orphan |

**"The tests pass" is not sufficient.** Task 17 is the claim, and it is the one that must be run and
observed by a person.

## Documentation to update

- [ ] `CLAUDE.md` §1 — Rule Zero's language constraint replaced; a new ADR supersedes it (this plan keeps `CLAUDE.md` canonical per decision 39)
- [ ] `CLAUDE.md` §2 — four invariants become eight (Tasks 6, 9, 10)
- [ ] `CLAUDE.md` §4 — Rust and Go tooling replaces `shellcheck` for the runner and dashboard
- [ ] `docs/decisions/0006-rust-over-posix-shell.md` — **new ADR**, superseding Rule Zero's first line
- [ ] `docs/reference/observed-behaviour.md` — the `pi`/`opencode` measurements and the real run (Tasks 8, 16, 17)
- [ ] `docs/guides/install.md` — tier configuration, the proven-adapter table, `jq` removed (Tasks 3, 8, 16)
- [ ] `docs/product/open-items.md` — anything the real run leaves open (Task 17)

## Risks

| Risk | Level | Mitigation |
|---|---|---|
| The cutover leaves a window with nothing runnable | **High** | The shell runner is not deleted until Task 17, after a real three-role run has succeeded. Tasks 1–16 add a binary beside a working runner |
| The pipeline ships with no shell oracle (three roles have no shell predecessor) | **High** | Named openly. Coverage is unit/integration tests plus Task 17's real failure-path runs |
| An adapter is written from documentation rather than a recorded transcript | Medium | `pi` was measured on 2026-08-19 and its transcript recorded; `opencode` ships unproven and is named as such in the guide |
| Local-time handling (quiet hours, daily cap) differs from the shell's `date` | Medium | `time` with `local-offset`; the ported `test_guards.sh` assertions include the boundary cases |
| `panic = "abort"` turns a bug into a wake that leaves nothing behind | Low | No `unwrap`/`expect` on reachable paths is a global constraint and a grep check; the journal is flushed per event |

## Out of scope

- **The deliver-skill preflight.** Plan 2 — it consumes `available()` and `models()` from Task 8.
- **The dashboard.** Plan 3 — it reads the journal Task 2 writes and nothing else.
- **The `codex` adapter.** `codex` is not installed on the development machine; the trait
  accommodates it and the design forbids shipping an unproven adapter as supported.
- **Parallel tasks per wake.** ADR-0001 stands.
- **Replacing `gh` or `git` with libraries.** Decided in the design, with the reason.
- **`autonomy.default`.** Still declared and unread — giving it more classes is a design question,
  not a port, and it moves to `open-items.md`.
