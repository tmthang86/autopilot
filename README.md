# Autopilot

An unattended delivery runner. A scheduler wakes it, it takes one task from a queue, drives a
coding agent to implement it, proves the result with the project's own verification commands,
commits, and exits.

It is deliberately not a daemon. Every run is a process that starts, does at most one task, and
dies. All state lives on disk, so sleeping the machine or exhausting the usage window costs
nothing — the next run picks up where the last one left off.

It does **not** survive a reboot on purpose. The loop runs only in a session you started; see
[ADR-0002](docs/decisions/0002-off-until-explicitly-started.md).

---

## Requirements

- macOS (the scheduler is a `launchd` job)
- `git`, `gh` (authenticated with `gh auth login`), `jq`, and the `claude` CLI
- a project that is a git repository with an `origin` remote on GitHub

The installer refuses a project without an `origin`, because every `gh` call the runner makes
names the repository explicitly — a job that can never reach the repository is worse than no job.

---

## Install

One command clones the runner and prepares a project. It writes the job and creates the labels,
but **starts nothing**:

```sh
curl -fsSL https://raw.githubusercontent.com/tmthang86/autopilot/main/install.sh \
  | sh -s -- /path/to/project
```

Prefer to read a script before running it? Download it first:

```sh
curl -fsSL https://raw.githubusercontent.com/tmthang86/autopilot/main/install.sh -o install.sh
sh install.sh /path/to/project
```

This installs the runner to `~/.local/share/autopilot/`, creates
`/path/to/project/.autopilot/config.json`, and creates the labels the queue reads. If the runner
is already present as a git checkout, the script fast-forwards it; a checkout that has diverged is
left untouched.

### Configure the project before starting

Open `config.json` and set `verify` to the commands your project must pass before anything is
committed:

```json
"verify": [
  {"name": "format", "cmd": "cargo fmt --check"},
  {"name": "lint",   "cmd": "cargo clippy --all-targets -- -D warnings"},
  {"name": "test",   "cmd": "cargo test"},
  {"name": "types",  "cmd": "pnpm typecheck"}
]
```

Then **commit it**. `config.json` is the contract between the project and the runner, and an
uncommitted edit is reverted by the first rejection path.

---

## Usage

The flow has two supervised ends — writing the plan and reviewing the results — and one
unattended middle.

### 1. Deliver a plan → issues

Run the `autopilot-deliver` skill (via the plugin, or read `skills/autopilot-deliver/SKILL.md`).
It validates that the plan is committed, prepares the project, proposes the issues, and creates
them in dependency order. Every issue it writes carries an `Intent:` line naming the documents
that authorise it — the runner refuses a task without one.

### 2. Start the loop

```sh
sh ~/.local/share/autopilot/runner/ctl.sh start  /path/to/project   # begins a session
sh ~/.local/share/autopilot/runner/ctl.sh status /path/to/project   # loaded? runner version? today's count?
```

The loop runs only in the session you started. It does not come back after a reboot, and `stop`
keeps it stopped across reboots. While it runs, each wake takes exactly one task:

```
select → claim → work → verify → settle (commit+push+close | reset+report)
```

Verification gates the commit. If the project's commands do not all pass, the tree is reset to
where the run began and nothing is committed.

### 3. Review the results

Run the `autopilot-review` skill (or read `skills/autopilot-review/SKILL.md`). It reads the logs,
`git log autopilot/main`, `blocked`/`needs-human` issues, and `state.json`, and produces a short
brief: what landed, what is waiting on a person, and whether `autopilot/main` is ready to merge
into `main`.

### 4. Stop the loop

```sh
sh ~/.local/share/autopilot/runner/ctl.sh stop /path/to/project    # ends it, and it stays ended
```

To pause without stopping the job, `touch /path/to/project/.autopilot/STOP`. The runner stands
down at the next tick until the file is removed.

Full instructions, including how to find a job orphaned by an upgrade or a repository rename:
[docs/guides/install.md](docs/guides/install.md).

---

## Design

```
scheduler   (launchd on macOS: StartInterval, started by hand)
    │
    ▼
run-once.sh
    ├─ guards     STOP file · lock · resume_after · daily cap · quiet hours
    ├─ select     queue → first task whose dependencies are all closed
    ├─ claim      mark in progress
    ├─ work       agent, headless, model and effort from the task's labels
    ├─ verify     run the project's verify commands — this gates the commit
    ├─ settle     green → commit + push + close;  red → reset --hard + report
    └─ classify   usage exhausted → set resume_after, no retry consumed
                  anything else  → consume one retry
```

The runner knows nothing about any specific project. Everything project-specific lives in a
config file the project commits:

```
~/.local/share/autopilot/           the deployed runner
~/.local/share/autopilot/jobs/      launchd job files, one per project
<project>/.autopilot/config.json    committed by the project — the contract
<project>/.autopilot/state.json     not committed
<project>/.autopilot/logs/          not committed
<project>/.autopilot/STOP           not committed — kill switch
```

This repository is also a Claude Code plugin (`/plugin marketplace add tmthang86/autopilot`). The
plugin ships `autopilot-deliver` and `autopilot-review`, and `runner/deploy.sh` copies `runner/` to
the stable path launchd points at, refusing to overwrite a git checkout.

---

## Status

Working. 335 assertions across 16 test files, exercised end to end under launchd on 2026-08-15
against a real repository with the real agent: issue claimed, work implemented, verification run,
commit pushed, issue closed, `main` untouched. The rejection path was proven too — a red suite
rewinds to the commit the run started from and leaves the issue open. That run also settled the
risk most worth worrying about: `gh` reaches its keychain token from inside a launchd job, which
would otherwise have looked exactly like a queue that is permanently empty.

**Read [docs/reference/observed-behaviour.md](docs/reference/observed-behaviour.md) before
changing anything here.** Every defect in it survived a passing test suite and was found only by
running against something real. Seven of them share one shape: *something did not work, and
everything reporting on it said otherwise*.

**Still unverified: a reboot.** That a stopped job stays stopped, and that a started one does not
come back, rest on the argument in
[ADR-0004](docs/decisions/0004-job-files-live-on-the-internal-disk.md) rather than on evidence.

---

## Safety

This runs unsupervised, usually with the agent's permission checks disabled. The protections are
structural rather than interactive:

- work happens on a dedicated branch; the project's main branch is never checked out
- verification gates the commit — a red suite resets the tree instead of committing
- `.autopilot/STOP` halts the loop at the next tick
- three consecutive failures create that STOP file automatically
- per-run spend ceiling and per-day task cap
- every command the agent runs is logged for review

Read [CLAUDE.md](CLAUDE.md) before changing anything. The constraints there exist because of what
this program is allowed to do — including the rule that no file in `runner/lib/` grows past the
point where one person can audit it in a sitting.

---

## Background

The design rationale, the surveyed alternatives, and the traps found by reading other
orchestrators' source are written up in [docs/decisions/](docs/decisions/) and
[docs/plans/](docs/plans/).