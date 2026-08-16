# The Rust runner: harnesses, tiers, and the role pipeline — implementation plan

> **Type:** Plan · **Date:** 2026-08-16 · **Status:** Awaiting approval
> **Prerequisite:** docs/plans/2026-08-16-planning-skill.md
> **Scope:** Sub-projects A, B and C of the multi-harness design, taken as one cutover — the harness
> abstraction, the tier ladder, the three-role pipeline, the journal, and the five declared-but-unread
> configuration keys that stop being cosmetic once a task is nine agent calls instead of one.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the POSIX shell runner with a Rust binary that can drive any of several agent
CLIs, at a model the project chooses per task tier, through an implement → test → review pipeline
whose every step is journalled.

**Architecture:** One binary, `autopilot`, with subcommands `run-once`, `status`, `start`, `stop`,
`install`. `git` and `gh` stay child processes. A `Harness` trait with one file per CLI is the only
place a CLI's name appears. The shell runner is frozen and used as a conformance oracle until parity
is proven, then deleted in one commit.

**Tech Stack:** Rust 2021, four crates only — `serde`, `serde_json`, `time` (with `local-offset`),
`libc`. No async runtime: the runner is sequential by ADR-0001 and `std::process::Command` is
sufficient. `git`, `gh`, and the agent CLIs remain external processes.

**Spec:** [`docs/design/2026-08-16-multi-harness-role-pipeline-design.md`](../design/2026-08-16-multi-harness-role-pipeline-design.md)
— decisions 1–16 and 24–25, and the sections *Why Rust*, *The harness adapter contract*, *The tier
ladder*, *Why three roles at all*, *The role pipeline*, *The branch model*, and *The journal*.

## Global Constraints

Copied from the spec and from `AGENTS.md` as it stands after plan 1. Every task implicitly includes
these.

- **Four crates, and no more**, without an ADR: `serde`, `serde_json`, `time`, `libc`. The
  replacement for Rule Zero is *a dependency tree an operator can enumerate*; a fifth crate is a
  decision, not a convenience.
- **No module exceeds ~200 lines.** The successor to the ~150-line shell rule.
- **No harness name appears outside `src/harness/`.** Grep-checkable, and a test.
- **The runner never references a skill or the dashboard.** Grep-checkable, and a test.
- `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` clean before every commit.
- **No `unwrap()` or `expect()` on any path a scheduled run can reach.** Tests only. A panic at
  3 a.m. is a wake that leaves nothing behind, which is this repository's most-repeated failure.
- Tests run against a **real throwaway git repository**. `gh` and every agent CLI are stubbed with
  fixture programs on `PATH`, and every fixture's shape is **recorded, never invented**.
- The **four invariants of `AGENTS.md` §2 plus the four added by the design** each carry at least one
  test that fails if the invariant is removed.
- The external contract does not change: `.autopilot/config.json`, `.autopilot/state.json`,
  `.autopilot/logs/`, the launchd plist, and every `gh` label name stay exactly as they are, except
  where the design says otherwise.

---

## Context

The runner works and does not vary. Every task is implemented, tested and reviewed by one invocation
of one hard-coded CLI. Making that vary means an abstraction over agent CLIs, a per-task choice of
model, and a pipeline of more than one call — and the design argues at length why those three arrive
together rather than separately.

The language changes at the same time and for a reason set out in the design: of the defects that
survived 166 passing tests, most were shell semantics rather than logic, and three of the four
classes are unrepresentable in Rust. The design in front of us — three roles, bounded rounds, four
harness adapters, per-call timeouts, cumulative ceilings, a branch lifecycle and a journal — is past
the size where shell is the honest choice.

## What we already know

Facts established before planning. No guesses.

- **The shell suite is 16 files and passes.** `sh tests/run.sh` reports `ALL PASS`.
- **Only five of those files can serve as a conformance oracle.** `test_run_once.sh`,
  `test_ctl.sh`, `test_install.sh`, `test_install_script.sh` and `test_deploy.sh` drive a process.
  The other eleven `source` shell functions directly and cannot call a Rust binary; their assertions
  are the specification of what the Rust unit tests must assert, and must be ported one by one.
- **Seven defects and their rules are recorded** in `docs/reference/observed-behaviour.md`. Every one
  is a finding about `gh`, `git`, or the agent CLI — which is why those stay child processes, and
  why each rule needs a Rust test rather than a fresh discovery.
- **Measured harness behaviour, 2026-08-16** (design, *Measured harness behaviour*): `claude` reports
  `total_cost_usd` on its final `result` event; `pi auth check --provider P --json` exits **0**
  regardless of readiness and answers `provider_not_found` for `ollama` and `lmstudio`;
  `opencode models` lists only reachable models.
- **On the development machine only `claude` is currently usable.** `opencode` fails every command
  on an invalid user config, `pi` has credentials for `deepseek` only, the LM Studio server is off,
  and `ollama` and `codex` are not installed. An adapter that has never run against its real CLI
  ships labelled unproven.
- **Five configuration keys are declared and read by nothing**: `agent.turn_timeout_s`,
  `pacing.max_attempts_per_issue`, `autonomy.default`, a cumulative spend ceiling, and the
  `launchctl` runs/last-exit reporting in `ctl.sh status`.
- **`gh` reaches its keychain token from inside a launchd job** (2026-08-15, observed). That is the
  single strongest reason not to replace `gh` with an HTTP client.

## Approach

Twenty-two tasks in five groups. Foundations first, because everything writes to the journal and
reads the config; then the pieces that have a shell predecessor to port; then the parts that are
genuinely new; then the cutover.

| Group | Tasks | What it establishes |
|---|---|---|
| Foundations | 1–3 | Cargo layout, gates, journal, logging, the conformance harness |
| Ported with a shell oracle | 4–11 | config, tier, state, label, queue, intent, guard, verify |
| The harness abstraction | 12–15 | The trait, the shared classifier, and three adapters |
| Genuinely new | 16–19 | Settlement with task branches, the role pipeline, prompts and verdicts, timeouts |
| Operator surface and cutover | 20–22 | status/start/stop, install, conformance and the deletion |

**Files created** — `runner/Cargo.toml`, `runner/src/*.rs` per the design's module list,
`runner/templates/prompt-{implement,test,review}.tmpl`, `tests/conformance.sh`.

**Files modified** — `runner/deploy.sh` (copies a binary, not a tree), `.claude-plugin/*`,
`runner/VERSION`, `docs/guides/install.md`, `AGENTS.md`, `docs/reference/observed-behaviour.md`.

**Files deleted at task 22, not before** — `runner/run-once.sh`, `runner/lib/`, `runner/ctl.sh`,
`runner/install-project.sh`, and the eleven function-level shell tests.

---

## Work breakdown

### Task 1: Cargo scaffold, the gates, and the two boundary tests

- **Done when:** `cargo build` produces `autopilot`, `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` are clean, and both grep-boundary tests exist and pass.
- **Verify:** `cd runner && cargo test --test boundaries` → 3 passed; `cargo clippy --all-targets -- -D warnings` → no warnings
- **Intent:** `docs/design/2026-08-16-multi-harness-role-pipeline-design.md` `AGENTS.md`
- **Depends on:** —
- **Tier:** light
- **Needs human:** no

**Files:**
- Create: `runner/Cargo.toml`, `runner/src/main.rs`, `runner/tests/boundaries.rs`
- Modify: `.gitignore` (add `runner/target/`)

**Interfaces:**
- Produces: the crate named `autopilot`, binary `autopilot`. Every later task adds modules to it.

- [ ] **Step 1: Write `runner/Cargo.toml`**

```toml
[package]
name = "autopilot"
version = "0.3.0"
edition = "2021"

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
time = { version = "0.3", features = ["formatting", "macros", "local-offset"] }
libc = "0.2"

[profile.release]
panic = "abort"
```

- [ ] **Step 2: Write the boundary tests first**

Create `runner/tests/boundaries.rs`. These encode invariants 7 and 8 of the design and must fail if
either is removed:

```rust
use std::fs;
use std::path::Path;

fn read_all(dir: &Path, out: &mut Vec<(String, String)>) {
    let entries = match fs::read_dir(dir) { Ok(e) => e, Err(_) => return };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() { read_all(&p, out); }
        else if p.extension().map_or(false, |x| x == "rs") {
            if let Ok(s) = fs::read_to_string(&p) {
                out.push((p.display().to_string(), s));
            }
        }
    }
}

#[test]
fn no_harness_name_outside_the_harness_module() {
    let mut files = Vec::new();
    read_all(Path::new("src"), &mut files);
    let mut offenders = Vec::new();
    for (path, body) in &files {
        if path.contains("src/harness/") { continue; }
        for name in ["claude", "opencode", "codex"] {
            if body.contains(name) { offenders.push(format!("{path}: {name}")); }
        }
        // "pi" is too short to grep for; it is matched as a whole word instead.
        for line in body.lines() {
            if line.split(|c: char| !c.is_alphanumeric()).any(|w| w == "pi") {
                offenders.push(format!("{path}: pi"));
            }
        }
    }
    assert!(offenders.is_empty(), "harness names leaked out of src/harness/: {offenders:?}");
}

#[test]
fn the_runner_never_references_a_skill_or_the_dashboard() {
    let mut files = Vec::new();
    read_all(Path::new("src"), &mut files);
    for (path, body) in &files {
        assert!(!body.contains("skills/"), "{path} references skills/");
        assert!(!body.contains("dashboard"), "{path} references the dashboard");
    }
}

#[test]
fn no_module_exceeds_two_hundred_lines() {
    let mut files = Vec::new();
    read_all(Path::new("src"), &mut files);
    let over: Vec<_> = files.iter()
        .filter(|(_, b)| b.lines().count() > 200)
        .map(|(p, b)| format!("{p}: {}", b.lines().count()))
        .collect();
    assert!(over.is_empty(), "modules over 200 lines: {over:?}");
}
```

- [ ] **Step 3: Write a `main.rs` that does nothing yet but parses `--project`**

```rust
fn main() -> std::process::ExitCode {
    let mut args = std::env::args().skip(1);
    let mut project: Option<String> = None;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--project" => project = args.next(),
            other => {
                eprintln!("usage: autopilot run-once --project <path>  (got {other})");
                return std::process::ExitCode::from(1);
            }
        }
    }
    match project {
        Some(_) => std::process::ExitCode::SUCCESS,
        None => { eprintln!("usage: autopilot run-once --project <path>"); std::process::ExitCode::from(1) }
    }
}
```

- [ ] **Step 4: Run the gates**

```sh
cd runner && cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test --test boundaries
```

Expected: no warnings; `3 passed`.

- [ ] **Step 5: Commit**

```sh
git add runner/Cargo.toml runner/src runner/tests .gitignore
git commit -m "feat: scaffold the Rust runner with its boundaries as tests

The two rules that keep this abstraction honest -- no harness name outside
src/harness/, and no reference to a skill or the dashboard -- are tests before
there is anything to break them. Their shell predecessor was added after the
fact and only ever checked one of the two."
```

---

### Task 2: `log.rs` and `journal.rs`

- **Done when:** log lines match the shell format byte for byte, and the journal appends one JSONL object per event with `role`, `tier`, `harness`, `model`, `cost_usd` and `cost_source`.
- **Verify:** `cargo test journal` → 7 passed; a written journal line parses with `jq -e .` and carries `cost_source`
- **Intent:** `docs/design/2026-08-16-multi-harness-role-pipeline-design.md`
- **Depends on:** Task 1
- **Tier:** standard
- **Needs human:** no

**Files:**
- Create: `runner/src/log.rs`, `runner/src/journal.rs`
- Test: `runner/src/journal.rs` (`#[cfg(test)]` module)

**Interfaces:**
- Produces:
  - `log::info(&str)`, `log::warn(&str)`, `log::error(&str)` — write `TIMESTAMP LEVEL message` to
    stderr and, when `AUTOPILOT_LOG_FILE` is set, append there too.
  - `journal::Journal::open(project: &Path, issue: u64) -> io::Result<Journal>`
  - `journal::Journal::append(&mut self, ev: Event) -> io::Result<()>`
  - `journal::Event` — a serde enum with variants `WakeStart`, `RoleStart`, `RoleEnd`, `Verdict`,
    `Verify`, `WakeEnd`, each carrying the fields the design's journal section lists.
  - `journal::CostSource` — `Reported | Estimated | Unknown`.

- [ ] **Step 1: Write the failing tests**

Add to `runner/src/journal.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_role_end_records_its_cost_source() {
        let dir = tempdir();
        let mut j = Journal::open(&dir, 42).expect("open");
        j.append(Event::RoleEnd {
            role: "implement".into(), round: 0, tier: "standard".into(),
            classify: "ok".into(), cost_usd: Some(0.42),
            cost_source: CostSource::Reported, duration_s: 312, lens: None,
        }).expect("append");
        let body = std::fs::read_to_string(dir.join("runs/42/journal.jsonl")).expect("read");
        let v: serde_json::Value = serde_json::from_str(body.trim()).expect("valid JSONL");
        assert_eq!(v["event"], "role_end");
        assert_eq!(v["cost_source"], "reported");
        assert_eq!(v["tier"], "standard");
    }

    #[test]
    fn an_unknown_cost_is_never_written_as_zero() {
        // Zero and "we do not know" are different numbers. Writing 0.0 for an
        // unmeasured cost is the dashboard-total-in-a-nicer-font failure.
        let dir = tempdir();
        let mut j = Journal::open(&dir, 1).expect("open");
        j.append(Event::RoleEnd {
            role: "test".into(), round: 0, tier: "light".into(), classify: "ok".into(),
            cost_usd: None, cost_source: CostSource::Unknown, duration_s: 5, lens: None,
        }).expect("append");
        let body = std::fs::read_to_string(dir.join("runs/1/journal.jsonl")).expect("read");
        let v: serde_json::Value = serde_json::from_str(body.trim()).expect("valid");
        assert!(v["cost_usd"].is_null(), "unknown cost must be null, not 0");
        assert_eq!(v["cost_source"], "unknown");
    }

    #[test]
    fn a_role_start_with_no_role_end_is_detectable() {
        // This is the whole point of the journal: a silently dead role.
        let dir = tempdir();
        let mut j = Journal::open(&dir, 7).expect("open");
        j.append(Event::RoleStart {
            role: "review".into(), round: 1, tier: "deep".into(),
            harness: "claude".into(), model: "opus".into(), lens: Some("partial-failure".into()),
        }).expect("append");
        let body = std::fs::read_to_string(dir.join("runs/7/journal.jsonl")).expect("read");
        let starts = body.lines().filter(|l| l.contains("\"role_start\"")).count();
        let ends = body.lines().filter(|l| l.contains("\"role_end\"")).count();
        assert_eq!((starts, ends), (1, 0), "an orphaned role must be visible as start-without-end");
    }
}
```

- [ ] **Step 2: Run and confirm they fail**

```sh
cd runner && cargo test journal
```

Expected: compilation errors — `Journal`, `Event`, `CostSource` do not exist.

- [ ] **Step 3: Implement `CostSource` and `Event`**

The two decisions that matter are in the derive attributes: `rename_all = "snake_case"` gives
`role_end` and `reported` without a second source of truth, and `skip_serializing_if` is **not** used
on `cost_usd`, because an absent field and a null field must be distinguishable by the dashboard.

```rust
use serde::Serialize;

#[derive(Serialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum CostSource { Reported, Estimated, Unknown }

#[derive(Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Event {
    WakeStart { issue: u64, tier: String },
    RoleStart { role: String, round: u32, tier: String, harness: String, model: String,
                lens: Option<String> },
    RoleEnd   { role: String, round: u32, tier: String, classify: String,
                cost_usd: Option<f64>, cost_source: CostSource, duration_s: u64,
                lens: Option<String> },
    Verdict   { role: String, round: u32, verdict: String, reason: String },
    Verify    { result: String, failed: Option<String> },
    WakeEnd   { outcome: String },
}
```

- [ ] **Step 4: Implement `Journal`**

Append-only, opened once per wake, flushed on every event — a buffered journal that loses its tail
when the process is killed is exactly useless for the failure it exists to detect.

```rust
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

pub struct Journal { file: File, wake: String }

impl Journal {
    pub fn open(project: &Path, issue: u64) -> io::Result<Journal> {
        let dir = project.join(format!("runs/{issue}"));
        fs::create_dir_all(&dir)?;
        let file = OpenOptions::new().create(true).append(true).open(dir.join("journal.jsonl"))?;
        Ok(Journal { file, wake: crate::log::wake_id() })
    }

    pub fn append(&mut self, ev: Event) -> io::Result<()> {
        let mut v = serde_json::to_value(&ev).map_err(io::Error::other)?;
        if let Some(o) = v.as_object_mut() {
            o.insert("ts".into(), serde_json::Value::String(crate::log::timestamp()));
            o.insert("wake".into(), serde_json::Value::String(self.wake.clone()));
        }
        writeln!(self.file, "{v}")?;
        self.file.flush()
    }
}
```

- [ ] **Step 5: Implement `log.rs` with the shell's exact format**

`tests/test_log.sh` records the format as `2026-08-16T11:00:58Z INFO message`. Match it, because the
conformance run in task 21 diffs log output.

- [ ] **Step 6: Run the tests and the gates**

```sh
cd runner && cargo test journal && cargo clippy --all-targets -- -D warnings
```

Expected: `7 passed`, no warnings.

- [ ] **Step 7: Commit**

```sh
git add runner/src/log.rs runner/src/journal.rs
git commit -m "feat: append-only journal that can show a role dying silently

A role_start with no matching role_end is the signature of the failure the
operator hit on 2026-08-16, where sub-agents died and nothing said so. It is
detectable with one query and needs no dashboard.

Every cost carries its source. An unmeasured cost is written null, never 0.0:
zero and 'we do not know' are different numbers, and summing them into one
figure is this repository's recurring failure in a nicer font."
```

---

### Task 3: The conformance harness

- **Done when:** `tests/conformance.sh` runs a named scenario against both the shell runner and the Rust binary in identical fixture repositories, and diffs exit code, log lines, git state and the `gh` calls each made.
- **Verify:** `sh tests/conformance.sh --list` → prints the scenario names; `sh tests/conformance.sh empty-queue` → `IDENTICAL`
- **Intent:** `docs/design/2026-08-16-multi-harness-role-pipeline-design.md` `tests/test_run_once.sh`
- **Depends on:** Task 1
- **Tier:** deep
- **Needs human:** no

**Files:**
- Create: `tests/conformance.sh`, `tests/conformance/` (one scenario per file)

**Interfaces:**
- Produces: `sh tests/conformance.sh <scenario>` → exit 0 and `IDENTICAL`, or exit 1 and a diff.
  Task 21 runs every scenario; tasks 4–20 add a scenario each time they port a behaviour.

- [ ] **Step 1: Write the harness**

The `gh` stub records every invocation to a file. That recording is the most valuable half of the
comparison: two runners can reach the same exit code having acted on different issues.

```sh
#!/bin/sh
# Run one scenario against both runners in identical repositories and diff what
# each observably did. Not `set -e`: a scenario that fails must still report.
set -u
ROOT=$(cd "$(dirname "$0")/.." && pwd)
SCEN=${1:-}
[ -n "$SCEN" ] || { ls "$ROOT/tests/conformance" | sed 's/\.sh$//'; exit 0; }

run_one() {   # run_one <impl> <outdir>
    _impl=$1; _out=$2
    mkdir -p "$_out/bin"
    # Every gh call is appended here before the stub answers.
    cat > "$_out/bin/gh" <<STUB
#!/bin/sh
printf '%s\n' "\$*" >> "$_out/gh-calls.txt"
. "$ROOT/tests/conformance/$SCEN.sh"
gh_stub "\$@"
STUB
    chmod +x "$_out/bin/gh"
    PROJ=$("$ROOT/tests/mkrepo.sh" "$_out/project")
    ( . "$ROOT/tests/conformance/$SCEN.sh"; setup "$PROJ" )
    case "$_impl" in
      shell) PATH="$_out/bin:$PATH" sh "$ROOT/runner/run-once.sh" --project "$PROJ" ;;
      rust)  PATH="$_out/bin:$PATH" "$ROOT/runner/target/debug/autopilot" run-once --project "$PROJ" ;;
    esac > "$_out/stdout.txt" 2> "$_out/stderr.txt"
    printf '%s' "$?" > "$_out/exit"
    git -C "$PROJ" log --oneline --all > "$_out/git-log.txt" 2>&1
    git -C "$PROJ" status --porcelain > "$_out/git-status.txt" 2>&1
    # Timestamps differ between runs by construction; strip them before diffing.
    sed 's/^[0-9T:Z-]*Z //' "$_out/stderr.txt" > "$_out/log-normalised.txt"
}

W=$(mktemp -d); trap 'rm -rf "$W"' EXIT
run_one shell "$W/shell"
run_one rust  "$W/rust"

FAIL=0
for f in exit log-normalised.txt git-log.txt git-status.txt gh-calls.txt; do
    if ! diff -u "$W/shell/$f" "$W/rust/$f" > "$W/diff-$f" 2>&1; then
        printf '=== DIVERGES: %s ===\n' "$f"; cat "$W/diff-$f"; FAIL=1
    fi
done
[ "$FAIL" -eq 0 ] && { echo IDENTICAL; exit 0; } || exit 1
```

- [ ] **Step 2: Extract `tests/mkrepo.sh` from the harness's `make_repo`**

Both runners need identical starting repositories, and `tests/harness.sh:44` already builds one.
Move that function into a standalone script both can call, and have `harness.sh` source it, so there
is one definition rather than two that drift.

- [ ] **Step 3: Write the first scenario, `empty-queue`**

```sh
# tests/conformance/empty-queue.sh
setup() {   # $1 = project path
    mkdir -p "$1/.autopilot"
    cp "$(dirname "$0")/../fixtures/config-minimal.json" "$1/.autopilot/config.json"
    git -C "$1" checkout -q -b autopilot/main
}
gh_stub() {
    case "$*" in
        *"issue list"*) echo '[]' ;;
        *"label list"*) echo '[{"name":"autopilot"}]' ;;
        *) echo '{}' ;;
    esac
}
```

- [ ] **Step 4: Run it against the shell runner twice to prove the harness itself is deterministic**

```sh
sh tests/conformance.sh empty-queue
```

Expected at this point: it **diverges**, because `autopilot run-once` is a stub that exits 0 without
logging. That is the correct first result — record it. The harness is proven when the same scenario
run shell-against-shell is `IDENTICAL`; add a `--both shell` flag if that is not otherwise
demonstrable.

- [ ] **Step 5: Commit**

```sh
git add tests/conformance.sh tests/conformance tests/mkrepo.sh tests/harness.sh
git commit -m "test: a conformance harness that diffs both runners' behaviour

Decision 15 makes the shell runner the reference specification for a single
cutover. A specification nobody can execute is a document; this makes it a test.

The gh stub records every call, and that recording is the more valuable half:
two runners can reach the same exit code having acted on different issues, and
this repository has already shipped a bug where exactly that happened."
```

---

### Task 4: `config.rs` — the two-layer load

- **Done when:** `config.json` and `.autopilot/tiers.local.json` load together, every tier name resolves to a binding, and a missing binding is a named error rather than a default.
- **Verify:** `cargo test config` → 11 passed, including one asserting an old config with `agent.default_model` fails to load with a message naming the key
- **Intent:** `docs/design/2026-08-16-multi-harness-role-pipeline-design.md` `tests/test_config.sh`
- **Depends on:** Task 2
- **Tier:** standard
- **Needs human:** no

**Files:**
- Create: `runner/src/config.rs`, `runner/tests/fixtures/config-minimal.json`

**Interfaces:**
- Produces:
  - `Config::load(project: &Path) -> Result<Config, ConfigError>`
  - `Config { project: Project, queue: Queue, tiers: Vec<String>, roles: Roles, pipeline: Pipeline,
    agent: Agent, verify: Vec<VerifyCmd>, pacing: Pacing, autonomy: Autonomy, bindings: BTreeMap<String, TierBinding> }`
  - `TierBinding { harness: String, model: String, effort: String, budget_usd: f64 }`
  - `ConfigError` — `NotFound(PathBuf)`, `Invalid{path, msg}`, `MissingKey(&'static str)`,
    `UnboundTier(String)`.

- [ ] **Step 1: Write the failing tests**

The three that matter are the ones a default would silently paper over:

```rust
#[test]
fn a_tier_with_no_binding_is_refused_by_name() {
    let p = fixture(r#"{"tiers":["light","deep"], ...}"#, r#"{"light":{...}}"#);
    match Config::load(&p) {
        Err(ConfigError::UnboundTier(t)) => assert_eq!(t, "deep"),
        other => panic!("expected UnboundTier(deep), got {other:?}"),
    }
}

#[test]
fn an_old_config_fails_loudly_rather_than_running_wrong() {
    // agent.default_model is gone. Silently defaulting would run every task on
    // the wrong model at 3 a.m. with nothing saying so.
    let p = fixture(r#"{"agent":{"default_model":"sonnet"}, ...}"#, "{}");
    let e = Config::load(&p).expect_err("must not load");
    assert!(format!("{e}").contains("tiers"), "the error must name what is missing: {e}");
}

#[test]
fn a_stored_zero_is_not_confused_with_absent() {
    // The jq `//` trap, ported. budget_usd 0.0 is a local model, not a default.
    let c = load_with_binding(r#"{"harness":"pi","model":"m","effort":"low","budget_usd":0}"#);
    assert_eq!(c.bindings["light"].budget_usd, 0.0);
}
```

- [ ] **Step 2: Run and confirm failure**

```sh
cd runner && cargo test config
```

Expected: compilation errors — `Config` does not exist.

- [ ] **Step 3: Implement with serde, no defaults on required fields**

Every field the shell's `_CFG_REQUIRED` listed is a non-`Option` field with no `#[serde(default)]`.
That is how "missing key" becomes a parse error naming the key, for free, instead of a check that
can be forgotten.

- [ ] **Step 4: Add the conformance scenario `bad-config`**

Both runners must refuse a config missing `verify` with a non-zero exit and a message naming it.

- [ ] **Step 5: Run tests, gates, and conformance**

```sh
cd runner && cargo test config && cargo clippy --all-targets -- -D warnings
cd .. && sh tests/conformance.sh bad-config
```

- [ ] **Step 6: Commit**

```sh
git add runner/src/config.rs runner/tests/fixtures tests/conformance/bad-config.sh
git commit -m "feat: two-layer config where a missing tier binding is fatal

Tier names are the project's contract and are committed; the machine's bindings
are local and gitignored, so a second machine re-runs preflight instead of two
machines fighting over one committed file.

An old config now fails to load naming what is missing. Defaulting it would run
every task on the wrong model at 3 a.m. with nothing saying so -- the failure
shape this repository has paid for seven times."
```

---

### Task 5: `tier.rs` — the ladder

- **Done when:** a tier name resolves to its binding, `tier_offset` moves up the ordered list, and the top tier resolves to itself rather than overflowing.
- **Verify:** `cargo test tier` → 6 passed
- **Intent:** `docs/design/2026-08-16-multi-harness-role-pipeline-design.md`
- **Depends on:** Task 4
- **Tier:** light
- **Needs human:** no

**Files:**
- Create: `runner/src/tier.rs`

**Interfaces:**
- Produces:
  - `tier::resolve(cfg: &Config, name: &str, offset: i32) -> Result<(&str, &TierBinding), ConfigError>`
  - Offset saturates at both ends: `resolve(cfg, top, 1)` returns the top tier.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn the_top_tier_offset_by_one_is_itself() {
    let c = cfg(&["light", "standard", "deep"]);
    let (name, _) = tier::resolve(&c, "deep", 1).expect("resolves");
    assert_eq!(name, "deep", "the ladder has a ceiling and must not overflow");
}

#[test]
fn an_unknown_tier_name_is_an_error_not_the_default() {
    let c = cfg(&["light", "deep"]);
    assert!(tier::resolve(&c, "medium", 0).is_err());
}
```

- [ ] **Step 2–4: Run, implement, re-run** as in Task 4.

- [ ] **Step 5: Commit**

```sh
git add runner/src/tier.rs
git commit -m "feat: an ordered tier ladder with a ceiling

tier_offset resolves 'one step up'. At the top it resolves to itself, so a
reviewer for the dearest tier is that tier rather than a panic or a silent
wrap to the cheapest one."
```

---

### Task 6: `state.rs`

- **Done when:** every assertion in `tests/test_state.sh` holds in Rust, including that a stored zero survives a round trip and that corrupt state is rebuilt from defaults.
- **Verify:** `cargo test state` → 10 passed, matching the shell file's 12 assertions minus the two that test shell quoting
- **Intent:** `docs/design/2026-08-16-multi-harness-role-pipeline-design.md` `tests/test_state.sh`
- **Depends on:** Task 2
- **Tier:** standard
- **Needs human:** no

**Files:**
- Create: `runner/src/state.rs`

**Interfaces:**
- Produces: `State::init(path)`, `State::get_num(&self, key, default) -> i64`,
  `State::set_num(&mut self, key, v)`, `State::bump(&mut self, key) -> i64`, `State::save(&self)`.

- [ ] **Step 1: Port every assertion from `tests/test_state.sh`**

Read that file and write one `#[test]` per assertion. It is the specification. The one that must not
be dropped is *"a stored zero is not confused with absent"* — it is the `jq //` trap and it changes
`backoff_step` from "no backoff" into "unknown".

- [ ] **Step 2–4: Run, implement, re-run.**

- [ ] **Step 5: Commit** with a body explaining that invariant 3 (state round-trips) now has typed
  round-trip tests rather than string ones.

---

### Task 7: `label.rs`

- **Done when:** the launchd job label is derived from the `origin` remote, not the directory name, and two projects with the same basename produce different labels.
- **Verify:** `cargo test label` → 5 passed
- **Intent:** `docs/design/2026-08-16-multi-harness-role-pipeline-design.md` `tests/test_label.sh`
- **Depends on:** Task 2
- **Tier:** light
- **Needs human:** no

- [ ] Port `tests/test_label.sh`'s assertions, implement, commit. The behaviour is unchanged; only
  the language is.

---

### Task 8: `queue.rs`

- **Done when:** every `gh` call names the repository explicitly, fetching is separate from choosing, a missing ready label is distinguished from an empty queue, and a failed claim stops the run.
- **Verify:** `cargo test queue` → 18 passed; `sh tests/conformance.sh missing-label` → IDENTICAL
- **Intent:** `docs/design/2026-08-16-multi-harness-role-pipeline-design.md` `docs/reference/observed-behaviour.md` `tests/test_queue.sh`
- **Depends on:** Task 4
- **Tier:** deep
- **Needs human:** no

**Files:**
- Create: `runner/src/queue.rs`, `runner/src/gh.rs`

**Interfaces:**
- Produces:
  - `gh::run(args: &[&str]) -> Result<String, GhError>` — captures **stderr as well as stdout** and
    returns gh's own message on failure. Every caller decides what to do; none discards it.
  - `Queue::init(project) -> Result<Queue, QueueError>` — derives the slug from `origin`, refuses a
    project with none.
  - `Queue::load(&mut self)`, `Queue::pick(&self) -> Option<u64>`, `Queue::field`,
    `Queue::has_label`, `Queue::claim`, `Queue::release`, `Queue::deps`.

- [ ] **Step 1: Port every assertion from `tests/test_queue.sh`** — 24 of them, including the
  recorded-fixture parse and the dependency-ordering cases.

- [ ] **Step 2: Add the four rules from `observed-behaviour.md` as named tests**

```rust
#[test] fn every_gh_call_names_the_repository() { /* stub records args; assert --repo present */ }
#[test] fn a_missing_ready_label_is_not_an_empty_queue() { /* issue list [] + label list without it */ }
#[test] fn loading_and_picking_are_separate_calls() { /* pick after load sees the cache */ }
#[test] fn a_failed_claim_returns_gh_stderr_to_the_caller() { /* stub fails; message propagates */ }
```

The first exists because every shell stub ignored `--repo`, so its absence was invisible — and a run
for project A could have closed issues in project B.

- [ ] **Step 3–5: Implement, run, add the `missing-label` conformance scenario, commit.**

---

### Task 9: `intent.rs`

- **Done when:** an issue body's `Intent:` line yields repo-relative paths, a path escaping the project root is refused, and a task with no valid intent is refused before any claim.
- **Verify:** `cargo test intent` → 14 passed; `sh tests/conformance.sh intent-refusal` → IDENTICAL
- **Intent:** `docs/decisions/0005-intent-binding.md` `tests/test_intent.sh`
- **Depends on:** Task 8
- **Tier:** standard
- **Needs human:** no

- [ ] Port `tests/test_intent.sh`. The containment test is the one that matters: a resolved path must
  stay under the project root, checked after canonicalisation, not before.

- [ ] Add the conformance scenario proving the refusal happens **before** `queue_claim` — assert the
  recorded `gh-calls.txt` contains no `issue edit` adding `status:in-progress`.

---

### Task 10: `guard.rs`

- **Done when:** lock, main-branch, quiet hours, daily cap, `STOP`, `resume_after` and the new `guard_tiers` all stand a run down with exit 0, and the lock is released on every path.
- **Verify:** `cargo test guard` → 16 passed; `sh tests/conformance.sh quiet-hours` and `stop-file` → IDENTICAL
- **Intent:** `docs/design/2026-08-16-multi-harness-role-pipeline-design.md` `tests/test_guards.sh`
- **Depends on:** Task 5, Task 8
- **Tier:** standard
- **Needs human:** no

**Interfaces:**
- Produces: `guard::all(cfg, state, project) -> GuardOutcome` where `GuardOutcome` is `Proceed` or
  `StandDown(reason)`; and `Lock` whose `Drop` releases, so no path can leak it.

- [ ] **Step 1: Port `tests/test_guards.sh`.**

- [ ] **Step 2: Add `guard_tiers`** — every tier in the ladder must report `available()`. Failure is
  `ProviderUnavailable`, which stands down with a backoff and consumes **no** retry budget.

- [ ] **Step 3: Prove the lock releases on a panic path**

```rust
#[test]
fn the_lock_is_released_even_when_the_run_panics() {
    let dir = tempdir();
    let _ = std::panic::catch_unwind(|| { let _l = Lock::acquire(&dir).unwrap(); panic!("boom") });
    assert!(Lock::acquire(&dir).is_ok(), "a leaked lock wedges every later wake");
}
```

The shell used `trap ... EXIT`; `Drop` is the equivalent and is harder to forget.

---

### Task 11: `verify.rs`

- **Done when:** commands run in order inside the project root, the first failure stops the rest and is named, and an empty verify list is refused rather than treated as success.
- **Verify:** `cargo test verify` → 9 passed
- **Intent:** `tests/test_verify.sh` `docs/design/2026-08-16-multi-harness-role-pipeline-design.md`
- **Depends on:** Task 4
- **Tier:** light
- **Needs human:** no

- [ ] Port `tests/test_verify.sh` exactly. The refusal of an empty list is invariant 1's floor: with
  no commands configured, "verified" would mean nothing at all.

---

### Task 12: `harness/mod.rs` — the trait and the shared classifier

- **Done when:** the `Harness` trait exists with the design's five methods, and the default `classify` uses exit status plus `probe()` to distinguish `TaskFailure` from `ProviderUnavailable`.
- **Verify:** `cargo test harness::default` → 6 passed
- **Intent:** `docs/design/2026-08-16-multi-harness-role-pipeline-design.md`
- **Depends on:** Task 2
- **Tier:** deep
- **Needs human:** no

**Interfaces:**
- Produces:

```rust
pub enum Classification { Ok, TaskFailure, ProviderUnavailable }

pub struct RunParams<'a> {
    pub model: &'a str, pub effort: &'a str, pub budget_usd: f64,
    pub timeout_s: u64, pub permission_mode: &'a str,
}

pub struct RunOutcome { pub status: Option<i32>, pub log: PathBuf,
                        pub cost_usd: Option<f64>, pub cost_source: CostSource,
                        pub duration_s: u64 }

pub trait Harness {
    fn name(&self) -> &'static str;
    fn available(&self) -> bool;
    fn models(&self) -> Vec<String>;
    fn run(&self, prompt: &str, cwd: &Path, log: &Path, p: &RunParams) -> io::Result<RunOutcome>;
    fn probe(&self) -> bool;
    fn classify(&self, out: &RunOutcome) -> Classification {
        if out.status == Some(0) { return Classification::Ok; }
        if self.probe() { Classification::TaskFailure } else { Classification::ProviderUnavailable }
    }
}

pub fn by_name(n: &str) -> Option<Box<dyn Harness>>;
```

- [ ] **Step 1: Write the tests against a fake harness defined in the test module**

```rust
#[test]
fn a_nonzero_exit_with_a_live_provider_is_a_task_failure() { /* probe true → TaskFailure */ }

#[test]
fn a_nonzero_exit_with_a_dead_provider_is_never_a_task_failure() {
    // Invariant 2, generalised. A dead endpoint must not consume a retry.
    /* probe false → ProviderUnavailable */
}
```

- [ ] **Step 2–4: Implement, run, commit.** The commit body records that the default classifier is
  what makes a new adapter correct from its first line and merely less precise.

---

### Task 13: `harness/claude.rs`

- **Done when:** it builds the same argument list the shell built, sends the prompt on stdin, parses `total_cost_usd` from the final `result` event, and keeps the two refinements from `observed-behaviour.md` §4 and §5.
- **Verify:** `cargo test harness::claude` → 9 passed, using the recorded fixture `tests/fixtures/claude-result-success.json`
- **Intent:** `docs/reference/observed-behaviour.md` `tests/test_agent.sh`
- **Depends on:** Task 12
- **Tier:** deep
- **Needs human:** no

- [ ] **Step 1: Port `tests/test_agent.sh`, then add the two trap tests explicitly**

```rust
#[test]
fn only_the_final_result_event_decides_the_outcome() {
    // A tool_result carrying is_error:true is an ordinary event inside a
    // successful task. Scanning the whole stream threw finished work away.
    let log = write(r#"{"type":"tool_result","is_error":true}
{"type":"result","is_error":false,"subtype":"success","total_cost_usd":0.05}"#);
    assert_eq!(classify(&log, Some(0)), Classification::Ok);
}

#[test]
fn is_error_false_is_not_treated_as_absent() {
    // jq's `//` returned the right-hand side for false as well as null, which
    // reported every successful run as an error.
    let log = write(r#"{"type":"result","is_error":false,"total_cost_usd":0.01}"#);
    assert_eq!(classify(&log, Some(0)), Classification::Ok);
}

#[test]
fn a_rate_limit_event_that_is_not_allowed_is_provider_unavailable() {
    let log = write(r#"{"type":"rate_limit_event","status":"throttled"}"#);
    assert_eq!(classify(&log, Some(1)), Classification::ProviderUnavailable);
}

#[test]
fn the_reported_cost_comes_from_the_final_event_and_is_marked_reported() {
    let out = parse_fixture("tests/fixtures/claude-result-success.json");
    assert_eq!(out.cost_usd, Some(0.0556587));
    assert_eq!(out.cost_source, CostSource::Reported);
}
```

- [ ] **Step 2–4: Implement, run, commit.**

---

### Task 14: `harness/opencode.rs` and `harness/pi.rs`

- **Done when:** both adapters exist, `pi`'s `available()` reads the **JSON status** rather than the exit code, and each ships marked unproven until it has run against its real CLI.
- **Verify:** `cargo test harness::pi` → 5 passed, including one asserting a `not_ready` payload with exit 0 is reported unavailable
- **Intent:** `docs/design/2026-08-16-multi-harness-role-pipeline-design.md` `docs/reference/observed-behaviour.md`
- **Depends on:** Task 12
- **Tier:** standard
- **Needs human:** yes

`Needs human: yes` — the design forbids shipping an adapter that has never executed against its real
CLI. Proving these needs the machine fixed first (`opencode`'s invalid user config, `pi`'s missing
credentials), and that is a person's action.

- [ ] **Step 1: The test that encodes the measured trap**

```rust
#[test]
fn pi_readiness_is_read_from_json_never_from_the_exit_code() {
    // Measured 2026-08-16: `pi auth check` exits 0 for a provider with no
    // credentials. Reading the exit status reports it ready.
    let stub = stub_pi(r#"{"status":"not_ready","provider":"anthropic","reason":"credentials_not_configured"}"#, 0);
    assert!(!Pi::with_binary(stub).available());
}
```

- [ ] **Step 2: Record a real transcript for each before writing its `classify`**

Run each CLI once against a throwaway repository and save its output under `tests/fixtures/`. **Do
not write the parser first.** A parser written before the shape is recorded is a shape invented.

- [ ] **Step 3: Mark unproven adapters in the guide**

`docs/guides/install.md` gains a table naming which adapters have executed against their real CLI
and on what date. An adapter absent from that table is not supported.

---

### Task 15: Prompt templates and verdict files

- **Done when:** three role templates exist, each role writes a verdict to the path the design names, and a missing or malformed verdict classifies as `TaskFailure`.
- **Verify:** `cargo test verdict` → 7 passed, including one asserting a missing file is never a pass
- **Intent:** `docs/design/2026-08-16-multi-harness-role-pipeline-design.md` `runner/templates/prompt.tmpl`
- **Depends on:** Task 12
- **Tier:** standard
- **Needs human:** no

**Files:**
- Create: `runner/templates/prompt-implement.tmpl`, `prompt-test.tmpl`, `prompt-review.tmpl`,
  `runner/src/verdict.rs`
- Delete: `runner/templates/prompt.tmpl` (superseded by the three)

- [ ] **Step 1: The test that carries invariant 5**

```rust
#[test]
fn a_missing_verdict_is_a_task_failure_never_a_pass() {
    let d = tempdir();
    assert_eq!(Verdict::read(&d.join("tester-verdict.json")), Err(VerdictError::Missing));
}

#[test]
fn a_malformed_verdict_is_a_task_failure_never_a_pass() {
    let p = write(r#"{"verdict":"probably fine"}"#);
    assert!(matches!(Verdict::read(&p), Err(VerdictError::Malformed(_))));
}
```

A reviewer that died mid-run must not become an approval.

- [ ] **Step 2: Write the three templates**

Each keeps the existing prompt's prohibitions and adds its own role. All three carry the same
prefix so the provider's prompt cache absorbs it, and all three name paths rather than pasting
excerpts — the rule the design takes from the companion project and decision 2 depends on.

The **test** template's distinguishing instruction: *you did not write this code and must not assume
the implementation is correct; test the task as the plan states it, not as the code implements it.*

The **review** template takes a lens name and is given **only that lens's question**.

- [ ] **Step 3–5: Implement `verdict.rs`, run, commit.**

---

### Task 16: `settle.rs` and the branch model

- **Done when:** work happens on `autopilot/task-<n>`, a green verify fast-forwards it into `autopilot/main` and deletes it, a pause pushes it and labels the issue `blocked`, and every rejection path rewinds to `START_SHA`.
- **Verify:** `cargo test settle` → 15 passed; `sh tests/conformance.sh verify-red` → IDENTICAL
- **Intent:** `docs/design/2026-08-16-multi-harness-role-pipeline-design.md` `tests/test_settle.sh` `docs/reference/observed-behaviour.md`
- **Depends on:** Task 8, Task 11
- **Tier:** deep
- **Needs human:** no

**Interfaces:**
- Produces: `settle::success`, `settle::failure`, `settle::pause`, and `settle::guard_branch(project, issue)`
  which now requires **exactly this issue's task branch**, not merely "not main".

- [ ] **Step 1: Port `tests/test_settle.sh`, then add the branch-model tests**

```rust
#[test]
fn the_agent_may_not_work_on_the_accumulation_branch() {
    // Invariant 6. Stronger than the shell's "not main": autopilot/main is now
    // written only by the fast-forward at the end.
    assert!(settle::guard_branch(&p, 7).is_err_on_branch("autopilot/main"));
}

#[test]
fn a_pause_keeps_the_work_and_a_failure_discards_it() {
    // Decision 7 against invariant 1: paused work survives on its pushed branch;
    // failed work is rewound to START_SHA so nothing unverified is kept.
}

#[test]
fn every_rejection_path_rewinds_to_the_recorded_sha_not_to_head() {
    // observed-behaviour.md §7: HEAD has already moved by then, and `reset --hard`
    // with no argument silently keeps the rejected commit.
}
```

- [ ] **Step 2: Keep the push rule** — push whatever `HEAD` is, always; if the push fails the issue
  stays **open** and says so. An issue closed while its work sits on one machine reads as delivered
  and is not.

- [ ] **Step 3–5: Implement, run conformance, commit.**

---

### Task 17: `pipeline.rs` — the three roles

- **Done when:** the implement → verify → test → review loop runs with `max_rounds`, review lenses each get a fresh call, a reviewer that has repaired does not gate its own fix, and every exit path is journalled.
- **Verify:** `cargo test pipeline` → 22 passed, one per exit path plus the decision-6 case
- **Intent:** `docs/design/2026-08-16-multi-harness-role-pipeline-design.md`
- **Depends on:** Task 15, Task 16, Task 10
- **Tier:** deep
- **Needs human:** no

**Interfaces:**
- Produces: `pipeline::run(cfg, project, issue, tier, journal) -> Outcome` where `Outcome` is
  `Merged | Failed | Paused | StoodDown`.

- [ ] **Step 1: Write one test per exit path, from the design's list**

Guard stand-down · provider unavailable · intent refusal · failed claim · implement failure ·
rounds exhausted · attempts exhausted · a lens rejecting · red final verify · success. Plus:

```rust
#[test]
fn a_reviewer_that_repaired_does_not_also_gate_its_repair() {
    // Decision 6 closes the hole decision 4 opened. After an adjudication round,
    // the tester's pass is the gate and the final review is skipped.
    let o = run_with(Scripted::tester_rejects_then_passes());
    assert_eq!(o.review_calls_after_repair, 0);
}

#[test]
fn each_review_lens_is_a_separate_invocation() {
    // Decision 24. One call with three headings is cheaper and weaker.
    let o = run_with(Scripted::all_pass(), &["plan-conformance", "partial-failure"]);
    assert_eq!(o.review_calls, 2);
}

#[test]
fn attempts_exhausted_pauses_rather_than_failing_again() {
    // The work stays on its branch and a coordinator decides. A task that can be
    // retried forever burns a usage window nightly with nobody deciding anything.
}
```

- [ ] **Step 2: Implement the loop exactly as the design's pseudocode reads**, with `verify()` at
  every boundary because it costs no tokens.

- [ ] **Step 3–5: Run, add the `three-role-happy-path` conformance scenario — noting it can only be
  compared against the shell for the *single-role* subset — and commit.**

Record in the commit that this is the first task with **no shell oracle**: the shell runner has one
role, so conformance can only prove the pipeline's single-role degenerate case. Everything else is
covered by unit tests and by task 22's real run.

---

### Task 18: `main.rs`, timeouts, and killing a hung agent properly

- **Done when:** `turn_timeout_s` kills a hung child **and its descendants**, `wake_timeout_s` ends the wake, and neither leaves the lock held.
- **Verify:** `cargo test timeout` → 6 passed, including one that spawns a script which forks a child and asserts both die
- **Intent:** `docs/design/2026-08-16-multi-harness-role-pipeline-design.md`
- **Depends on:** Task 17
- **Tier:** deep
- **Needs human:** no

- [ ] **Step 1: The test that justifies the `libc` dependency**

```rust
#[test]
fn a_timeout_kills_the_agents_children_too() {
    // Without a process group, killing `claude` leaves whatever it spawned
    // running. A local model that hangs then leaks processes every wake.
    let script = write_sh("sh -c 'sleep 300' & sleep 300");
    let out = run_with_timeout(&script, 1);
    assert!(out.timed_out);
    assert_eq!(surviving_children(&out.pgid), 0);
}
```

- [ ] **Step 2: Implement with `pre_exec` + `setsid`, and `killpg` on timeout**

```rust
use std::os::unix::process::CommandExt;
unsafe { cmd.pre_exec(|| { libc::setsid(); Ok(()) }); }
// on timeout:
unsafe { libc::killpg(pgid, libc::SIGTERM); }
// then SIGKILL after a grace period
```

- [ ] **Step 3: Wire `main.rs`** — the run-once flow from the design, with the lock as a `Drop`
  guard so no exit path can leak it.

- [ ] **Step 4–5: Run, commit.** The body records that `turn_timeout_s` was declared and unread for
  the whole life of the shell runner, and that with a local model in the ladder a hung child would
  have held the lock and blocked every later wake.

---

### Task 19: `autopilot status | start | stop`

- **Done when:** `status` reports `launchctl` runs and last exit code, compares the deployed `VERSION` against the plugin's, and names orphaned roles from the journal; `start` and `stop` verify with `launchctl print` rather than assuming.
- **Verify:** `sh tests/conformance.sh ctl-status` → IDENTICAL for the shared subset; `cargo test ctl` → 8 passed
- **Intent:** `docs/reference/observed-behaviour.md` `tests/test_ctl.sh`
- **Depends on:** Task 2
- **Tier:** standard
- **Needs human:** no

- [ ] **Step 1: Port `tests/test_ctl.sh`.**

- [ ] **Step 2: Close the three gaps the shell left**

```rust
#[test] fn status_reports_a_loaded_job_that_has_never_run() {
    // `runs = 0` well after the interval is the EX_CONFIG signature, and
    // observed-behaviour.md names this as "the check that would catch it".
}
#[test] fn status_reports_a_stale_deployed_runner() { /* VERSION comparison */ }
#[test] fn status_names_a_role_that_started_and_never_ended() { /* from the journal */ }
```

---

### Task 20: `autopilot install`

- **Done when:** it writes `config.json`, the gitignore entry and the plist, creates the labels idempotently, and refuses before writing anything when `origin` is missing.
- **Verify:** `sh tests/conformance.sh install` → IDENTICAL; `cargo test install` → 12 passed
- **Intent:** `docs/reference/observed-behaviour.md` `tests/test_install.sh`
- **Depends on:** Task 7, Task 8
- **Tier:** standard
- **Needs human:** no

- [ ] **Step 1: Port `tests/test_install.sh`.**

- [ ] **Step 2: Keep the measured `gh label create` rule** — only a stderr containing
  `already exists` is benign; anything else fails the install with gh's message verbatim. Treating
  every non-zero exit as "already exists" reports a clean install over a repository with no labels.

- [ ] **Step 3: Refuse a project with no `origin` before writing anything**, because `queue::init`
  treats it as fatal on every tick and not even a STOP file quiets it — a green install that could
  never run one task.

---

### Task 21: Run every conformance scenario

- **Done when:** every scenario reports `IDENTICAL`, or a divergence is recorded with the reason it is intended.
- **Verify:** `for s in $(sh tests/conformance.sh --list); do sh tests/conformance.sh "$s"; done` → every line `IDENTICAL`, or a documented exception
- **Intent:** `docs/design/2026-08-16-multi-harness-role-pipeline-design.md` `tests/conformance.sh`
- **Depends on:** Task 20
- **Tier:** deep
- **Needs human:** no

- [ ] **Step 1: Run them all and record the results in a table.**

- [ ] **Step 2: For each intended divergence, write down why**

Some are intended and must be named rather than explained away at review time: the task-branch model
changes `git-log.txt`, three roles change the number of agent invocations, and `tier:` labels replace
`model:`/`effort:`. **An unexplained divergence is a bug until proven otherwise** — that direction of
doubt is the point of the exercise.

- [ ] **Step 3: Commit the results table into `docs/reference/observed-behaviour.md`.**

---

### Task 22: The cutover

- **Done when:** the shell runner and its eleven function-level tests are deleted, `deploy.sh` copies a binary, the plugin ships it, and one real task has run end to end through three roles against a real repository.
- **Verify:** in a throwaway repository, one issue with a valid `Intent:` and `tier:` label runs implement → test → review, verification passes, the task branch fast-forwards into `autopilot/main`, the issue closes, and `.autopilot/runs/<n>/journal.jsonl` shows a matching `role_end` for every `role_start`
- **Intent:** `docs/design/2026-08-16-multi-harness-role-pipeline-design.md` `docs/reference/observed-behaviour.md`
- **Depends on:** Task 21
- **Tier:** deep
- **Needs human:** yes

`Needs human: yes` — this is the Definition-of-Done clause that asks for a real run, and it is the
clause this repository has skipped before.

- [ ] **Step 1: Exercise it for real, before deleting anything**

Create a throwaway repository with a real remote, install, add one issue, run once. Read the journal
and confirm three `role_start` events each have a `role_end`.

- [ ] **Step 2: Exercise the failure paths for real too**

A red verify, a tester rejection that the reviewer repairs, and a rounds-exhausted pause. The pause
is the one to watch: confirm the work is **on the pushed task branch** and the issue carries
`blocked` with the argument in a comment.

- [ ] **Step 3: Only now, delete the shell runner**

```sh
git rm -r runner/run-once.sh runner/lib runner/ctl.sh runner/install-project.sh \
          runner/templates/prompt.tmpl \
          tests/test_agent.sh tests/test_config.sh tests/test_guards.sh tests/test_intent.sh \
          tests/test_label.sh tests/test_log.sh tests/test_queue.sh tests/test_settle.sh \
          tests/test_state.sh tests/test_verify.sh
```

`tests/conformance.sh` goes with it — it has no second implementation to compare against, and a
conformance suite with one side is a suite that always passes.

- [ ] **Step 4: Update `deploy.sh`, the plugin manifests, `VERSION`, and the guide.**

- [ ] **Step 5: Record what the real run taught in `docs/reference/observed-behaviour.md`**, with the
  date and how it was observed — including anything that went wrong, which is the point of the file.

- [ ] **Step 6: Commit the cutover as one commit**, with a body stating what was exercised, on what
  date, and what remains unverified.

---

## Verification

| Check | Command | Passing looks like |
|---|---|---|
| Unit and integration | `cd runner && cargo test` | all green, ~200 tests |
| Lint and format | `cargo clippy --all-targets -- -D warnings && cargo fmt --check` | no output |
| Boundaries | `cargo test --test boundaries` | 3 passed |
| Conformance | every scenario | `IDENTICAL`, or a divergence with a written reason |
| No panics on reachable paths | `grep -rn 'unwrap()\|expect(' runner/src` | only inside `#[cfg(test)]` |
| Exercised for real | Task 22 | three roles, a green merge, and a journal with no orphan |

**"The tests pass" is not sufficient.** Task 22 is the claim, and it is the one that must be run and
observed.

## Documentation to update

- [ ] `AGENTS.md` §1 — Rule Zero's language constraint replaced; a new ADR supersedes it (Task 1)
- [ ] `AGENTS.md` §2 — four invariants become eight (Tasks 10, 15, 16)
- [ ] `AGENTS.md` §4 — Rust and Go tooling replaces `shellcheck` (Task 1)
- [ ] `docs/decisions/0006-rust-over-posix-shell.md` — **new ADR**, superseding Rule Zero's first line (Task 1)
- [ ] `docs/reference/observed-behaviour.md` — the three harness measurements, the conformance results, and the real run (Tasks 14, 21, 22)
- [ ] `docs/guides/install.md` — tier configuration, the proven-adapter table, `jq` removed (Tasks 4, 14, 22)
- [ ] `docs/README.md` status table — this plan's row (Task 22)
- [ ] `docs/product/open-items.md` — anything the real run leaves open (Task 22)

## Risks

| Risk | Level | Mitigation |
|---|---|---|
| The cutover leaves a window with nothing runnable | **High** | The shell runner is not deleted until Task 22, after a real three-role run has succeeded. Tasks 1–21 add a binary beside a working runner |
| Conformance can only cover the single-role subset, so the pipeline ships with no oracle | **High** | Named openly in Task 17. The pipeline's coverage is unit tests plus Task 22's real failure-path runs, and Task 22 exercises the pause and the rejection paths specifically because they have no oracle |
| An adapter is written from documentation rather than a recorded transcript | Medium | Task 14 records the transcript **before** writing the parser, and `install.md` lists which adapters have actually run. An adapter absent from that table is unsupported |
| Local-time handling (quiet hours, daily cap) differs from the shell's `date` | Medium | `time` with `local-offset`; the ported `test_guards.sh` assertions include the boundary cases, and a conformance scenario pins one |
| Twenty-two tasks is long enough that the design drifts from the code | Medium | Each task's `Intent:` names the design, and the reviewer's `plan-conformance` lens exists for exactly this |
| `panic = "abort"` turns a bug into a wake that leaves nothing behind | Low | No `unwrap`/`expect` on reachable paths is a global constraint and a grep check; the journal is flushed per event so the last line before an abort survives |

## Out of scope

- **The deliver-skill preflight.** Plan 3 — it consumes `available()` and `models()` from Task 12.
- **The dashboard.** Plan 4 — it reads the journal Task 2 writes and nothing else.
- **`codex` and local-model adapters.** The trait accommodates them; neither can be run on the
  development machine, and the design forbids shipping an unproven adapter as supported.
- **Parallel tasks per wake.** ADR-0001 stands.
- **Replacing `gh` or `git` with libraries.** Decided in the design, with the reason.
- **`autonomy.default`.** Still declared and unread. It is a single class today; giving it more
  classes is a design question, not a port, and it moves to `open-items.md` in Task 22.
