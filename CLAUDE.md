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
| POSIX shell only, no runtime dependencies beyond `git`, `gh`, `jq`, and the agent CLI | Anything the operator has to trust but cannot read is a liability |
| No file in `lib/` exceeds ~150 lines | A file you can hold in your head is a file you can audit |
| No network calls except through `gh` and the agent CLI | The set of things that can reach out stays enumerable |
| Never write outside the project root or `.autopilot/` | Enforced by an explicit path check, not by convention |
| Never operate on the project's main branch | Checked before every run, not assumed |

## 2. The four invariants

These hold on every code path, including error paths. A change that breaks one is a bug
regardless of what it enables.

1. **Verification gates the commit.** If the project's verify commands do not all pass, the
   working tree is reset and nothing is committed. There is no flag to skip this.
2. **Usage exhaustion is not a task failure.** Running out of the agent's usage window must
   never consume a task's retry budget or mark it failed. Classify before you back off.
3. **State round-trips.** Anything written to `state.json` must be readable back into the same
   shape by the next run. A process that exits after every task has no memory other than this
   file; write-only state is not state.
4. **The runner executes intent, it never authors it.** It implements tasks that a human
   approved. When it meets a decision the task does not cover, it records the question and
   stops that task.

## 3. Portability is the product

The runner is installed once and shared by every project on the machine. All project-specific
knowledge lives in `<project>/.autopilot/config.yml`, which the project commits.

Four things, and only these four, are project-specific:

- **verify** — the commands that must pass before a commit
- **autonomy** — which classes of task may be closed by the runner and which must stop for a human
- **pacing** — interval, daily cap, quiet hours, spend ceiling
- **queue** — which label means eligible, how dependencies are declared

Portability now includes queue preparation: `install-project.sh` creates the
labels the runner queries, so a project is not required to have been prepared
by hand before the runner is pointed at it.

**If you find yourself writing a project's name, language, or build tool into `lib/`, stop.**
It belongs in the config schema instead.

## 4. Testing

- Every function in `lib/` that makes a decision has a test. Functions that only print do not.
- Tests run against a **real throwaway git repository** created in a temp directory, never
  against mocks of git. Git's behaviour is the thing being relied on.
- `gh` and the agent CLI are stubbed with fixture scripts on `PATH`. Record real output shapes
  into `tests/fixtures/` — never invent a JSON shape to make a test pass.
- Every invariant in §2 has at least one test that fails if the invariant is removed.
- `shellcheck` clean before commit.

## 5. Documentation

- `docs/plans/` — written before code, following `_template.md`
- `docs/design/` — validated designs, written before the plans that implement them
- `docs/decisions/` — ADRs for expensive or contested choices; never edit an accepted one,
  supersede it instead
- `docs/guides/` — install, configure, tune, troubleshoot
- `docs/reference/observed-behaviour.md` — **highest priority.** Anything learned the hard way
  about the agent CLI, `gh`, or usage limits goes here the moment it is learned, with the date
  and how it was observed. This file is the reason the next person does not pay twice.

## 6. Commits

Conventional Commits. One commit is one coherent change including its docs. Commit bodies
state **why**; the diff already states what.

## 7. Definition of Done

- [ ] Built to the approved plan, or the plan was revised and re-approved
- [ ] `shellcheck` clean, tests written and **actually run**, output read
- [ ] Every §2 invariant still holds on error paths, not just the happy path
- [ ] Docs updated in the same commit
- [ ] Exercised against a real repository, not only in tests

If any box is unchecked, report it as **not done** and say which one.
