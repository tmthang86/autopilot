# Autopilot — Engineering Rules

A portable, unattended delivery runner. A scheduler wakes it, it takes one task from a queue,
drives a coding agent to implement it, proves the result with the project's own verification
commands, commits, and exits. It knows nothing about any specific project.

---

## 1. Rule Zero: this code runs unsupervised with elevated permissions

This runner is expected to execute on a machine while nobody is watching, often with the
agent's permission checks disabled. That single fact drives every rule below.

**The operator must be able to read this entire codebase in one sitting.** If a change makes
that harder, the change is wrong even if it works.

| Constraint | Why |
|---|---|
| Rust, with a dependency tree an operator can enumerate (`serde`, `serde_json`, `time`, `libc` only). `git` and `gh` remain child processes | Anything the operator has to trust but cannot read is a liability |
| No file in `runner/src/` exceeds ~200 lines | A file you can hold in your head is a file you can audit |
| No network calls except through `gh` and the harness CLIs | The set of things that can reach out stays enumerable |
| Never write outside the project root or `.autopilot/` | Enforced by an explicit path check, not by convention |
| Never operate on the project's main branch | Checked before every run, not assumed |

## 2. The eight invariants

These hold on every code path, including error paths. A change that breaks one is a bug
regardless of what it enables. The first four are the originals, reworded by the multi-harness
design; the last four are added by it.

1. **No unverified work reaches `autopilot/main`.** The project's verify commands must all pass
   before the fast-forward merge. A red final verify rewinds to the recorded start SHA; there is
   no flag to skip this. (Paused work may be committed as a `WIP:` commit on its task branch, but
   never merged.)
2. **Usage exhaustion is not a task failure.** Running out of the agent's usage window — or a dead
   provider endpoint, or an expired token — must never consume a task's retry budget or mark it
   failed. Classify before you back off.
3. **State round-trips.** Anything written to `state.json` must be readable back into the same
   shape by the next run. This extends to the journal and to paused task branches.
4. **The runner executes intent, it never authors it.** It implements tasks that a human approved.
   When it meets a decision the task does not cover, it records the question and pauses that task.
5. **A missing or malformed verdict is never a pass.** A reviewer that died mid-run must not become
   an approval.
6. **The agent never works on `autopilot/main`, only on a task branch.**
7. **No harness name appears outside `runner/src/harness/`.**
8. **The runner never references a skill or the dashboard.**

## 3. Portability is the product

The runner is installed once and shared by every project on the machine. All project-specific
knowledge lives in `<project>/.autopilot/config.json`, which the project commits.

Five things, and only these five, are project-specific:

- **verify** — the commands that must pass before a commit
- **autonomy** — which classes of task may be closed by the runner and which must stop for a human
- **pacing** — interval, daily cap, quiet hours, spend ceiling
- **queue** — which label means eligible, how dependencies are declared
- **intent** — how a task names the documents that authorise it (`queue.intent_marker`,
  default `Intent:`)

Portability now includes queue preparation: `install-project.sh` creates the
labels the runner queries, so a project is not required to have been prepared
by hand before the runner is pointed at it.

**If you find yourself writing a project's name, language, or build tool into `runner/lib/`,
stop.** It belongs in the config schema instead.

## 4. Testing

- Every function in `runner/src/` that makes a decision has a test. Functions that only print do
  not.
- Tests run against a **real throwaway git repository** created in a temp directory, never
  against mocks of git. Git's behaviour is the thing being relied on.
- `gh` and the harness CLIs are stubbed with fixture scripts on `PATH`. Record real output shapes
  into `tests/fixtures/` — never invent a JSON shape to make a test pass.
- Every invariant in §2 has at least one test that fails if the invariant is removed. Invariants 7
  and 8 are grep tests.
- `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check` clean before commit for the
  runner; `go vet` and `gofmt -l` clean for the dashboard; `shellcheck` clean for any shell that
  remains (the skills and the test harness).

## 5. Documentation sync rules

**A stale document is worse than no document**, because it makes the reader confidently wrong.

The table below is binding. Change something on the left, and **you must update the right in the
same commit**. Never defer a documentation update.

| When you change… | You must update |
|---|---|
| A module under `runner/src/` | The architecture section of the design it implements |
| The config schema in `runner/templates/config-tiered.json` | `docs/guides/install.md` and the tier section of the current design |
| An invariant in §2 | The design that states it, plus the test that fails when it is removed |
| Behaviour of `gh`, `git`, or a harness CLI learned the hard way | `docs/reference/observed-behaviour.md` ← **highest priority** |
| A pick of tool, technique, or a reversed decision | A new ADR under `docs/decisions/` |
| Anything an operator does differently | `docs/guides/install.md` |
| A document added, removed, or finished | The status table in `docs/README.md` |
| A question raised with no answer, or debt taken on | `docs/product/open-items.md` |

**"If it cost you, write it down."** Every hour lost to surprising CLI behaviour goes into
`docs/reference/observed-behaviour.md` immediately, with the date and how it was observed. That is
worth more than the next feature.

Before calling anything done, walk this table row by row.

## 6. Documentation

- `docs/plans/` — written before code, following `_template.md`
- `docs/design/` — validated designs, written before the plans that implement them
- `docs/decisions/` — ADRs for expensive or contested choices; never edit an accepted one,
  supersede it instead
- `docs/guides/` — install, configure, tune, troubleshoot
- `docs/reference/observed-behaviour.md` — **highest priority.** Anything learned the hard way
  about the agent CLI, `gh`, or usage limits goes here the moment it is learned, with the date
  and how it was observed. This file is the reason the next person does not pay twice.

## 7. Commits

Conventional Commits. One commit is one coherent change including its docs. Commit bodies
state **why**; the diff already states what.

## 8. Definition of Done

- [ ] Built to the approved plan, or the plan was revised and re-approved
- [ ] `cargo clippy`/`cargo test` (Rust), `go vet`/`go test` (Go), or `shellcheck` (shell) clean —
  tests written and **actually run**, output read
- [ ] Every §2 invariant still holds on error paths, not just the happy path
- [ ] Docs updated in the same commit
- [ ] Exercised against a real repository, not only in tests

If any box is unchecked, report it as **not done** and say which one.
