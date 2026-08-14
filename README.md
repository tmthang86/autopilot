# Autopilot

An unattended delivery runner. A scheduler wakes it, it takes one task from a queue, drives a
coding agent to implement it, proves the result with the project's own verification commands,
commits, and exits.

It is deliberately not a daemon. Every run is a process that starts, does at most one task, and
dies. All state lives on disk. That is what lets it survive sleep, reboot, and an exhausted
usage window without supervision.

## What it is for

Handing an approved plan to a machine and letting it work through the plan while you are not
there — including overnight, and including across the boundary where the agent's usage window
runs out and reopens.

## What it is not for

Authoring plans, deciding architecture, or verifying anything a human has to look at. It
implements intent that already exists and stops when it meets a decision the task does not
cover.

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

The runner is installed once per machine. Everything project-specific lives in a config file
that the project itself commits:

```
~/.local/share/autopilot/        this repository — installed once, shared
<project>/.autopilot/config.yml  committed by the project — the contract
<project>/.autopilot/state.json  not committed
<project>/.autopilot/STOP        not committed — kill switch
```

Porting to a new project means writing that config file. Nothing in `lib/` knows what language
a project is written in.

## Status

Working, covered by 187 tests, and exercised end to end on 2026-08-14 against a real private
repository with the real agent: issue claimed, work implemented, verification run, commit pushed,
issue closed, `main` untouched. The rejection path was proven too — a red suite rewinds to the
commit the run started from and leaves the issue open.

Six runs were needed to get there. Seven defects survived the passing test suite and were found
only by running for real, including one that turned the verification gate off whenever the agent
committed its own work. All seven are written up in
[docs/reference/observed-behaviour.md](docs/reference/observed-behaviour.md), which is the first
thing to read before changing anything here.

**Still unverified:** a run driven by launchd rather than by hand, and whether `stop` really
survives a reboot.

```sh
sh install-project.sh /path/to/project   # writes the job, starts nothing
sh ctl.sh start /path/to/project          # begins a session
sh ctl.sh stop  /path/to/project          # ends it, across reboots too
```

**The loop runs only in a session you started.** It does not resume by itself after a reboot; see
[ADR-0002](docs/decisions/0002-off-until-explicitly-started.md) for why, and what that costs.

Full instructions: [docs/guides/install.md](docs/guides/install.md).

## Safety

This runs unsupervised, usually with the agent's permission checks disabled. The protections
are structural rather than interactive:

- work happens on a dedicated branch; the project's main branch is never checked out
- verification gates the commit — a red suite resets the tree instead of committing
- `.autopilot/STOP` halts the loop at the next tick
- three consecutive failures create that STOP file automatically
- per-run spend ceiling and per-day task cap
- every command the agent runs is logged for review

Read [CLAUDE.md](CLAUDE.md) before changing anything. The constraints there exist because of
what this program is allowed to do.

## Background

The design rationale, the surveyed alternatives, and the traps found by reading other
orchestrators' source are written up in [docs/decisions/](docs/decisions/) and the plan.
