# Autopilot

An unattended delivery runner. A scheduler wakes it, it takes one task from a queue, drives a
coding agent to implement it, proves the result with the project's own verification commands,
commits, and exits.

It is deliberately not a daemon. Every run is a process that starts, does at most one task, and
dies. All state lives on disk, so sleeping the machine or exhausting the usage window costs
nothing — the next run picks up where the last one left off.

It does **not** survive a reboot on purpose. The loop runs only in a session you started; see
[ADR-0002](docs/decisions/0002-off-until-explicitly-started.md).

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
~/.local/share/autopilot/           the deployed runner
~/.local/share/autopilot/runner/    the only part a scheduled run needs
~/.local/share/autopilot/jobs/      launchd job files, one per project
<project>/.autopilot/config.json    committed by the project — the contract
<project>/.autopilot/state.json     not committed
<project>/.autopilot/logs/          not committed
<project>/.autopilot/STOP           not committed — kill switch
```

Porting to a new project means writing that config file and letting the installer create the
labels the queue reads. Nothing in `runner/lib/` knows what language a project is written in.

## Installing

```sh
git clone https://github.com/tmthang86/autopilot.git ~/.local/share/autopilot
sh ~/.local/share/autopilot/runner/install-project.sh /path/to/project   # writes the job, starts nothing
sh ~/.local/share/autopilot/runner/ctl.sh start /path/to/project          # begins a session
sh ~/.local/share/autopilot/runner/ctl.sh stop  /path/to/project          # ends it, and it stays ended
```

The installer refuses a project with no `origin` remote, because the runner cannot work without
one and a job that can never run is worse than no job at all. It creates the nine labels the
queue reads, and it never starts anything — enabling a program that runs with permission checks
disabled is your decision, not the installer's.

**The loop runs only in a session you started.** It does not resume by itself after a reboot; see
[ADR-0002](docs/decisions/0002-off-until-explicitly-started.md) for why, and what that costs.

This repository also carries Claude Code plugin manifests, so `/plugin marketplace add
tmthang86/autopilot` works. It is not yet useful: the plugin ships no skills, and nothing copies
`runner/` out to the stable path that installed launchd jobs point at. That deployment step is
[designed](docs/design/2026-08-15-skill-layer-design.md) and not built. Clone until it lands.

Full instructions, including how to find a job orphaned by an upgrade or a repository rename:
[docs/guides/install.md](docs/guides/install.md).

## Status

Working. 269 assertions across 13 test files, and exercised end to end under launchd on
2026-08-15 against a real repository with the real agent: issue claimed, work implemented,
verification run, commit pushed, issue closed, `main` untouched. The rejection path was proven
too — a red suite rewinds to the commit the run started from and leaves the issue open. That run
also settled the risk most worth worrying about: `gh` reaches its keychain token from inside a
launchd job, which would otherwise have looked exactly like a queue that is permanently empty.

**Read [docs/reference/observed-behaviour.md](docs/reference/observed-behaviour.md) before
changing anything here.** It is the most valuable document in the repository. Every defect in it
survived a passing test suite and was found only by running against something real.

Seven of them share one shape, and it is the shape to watch for: **something did not work, and
everything reporting on it said otherwise.** A start command that discarded the scheduler's exit
status. A missing label reported as an empty queue. A job that loaded but could not run. Every
`gh` failure printed as "already exists". A bare `var=$(gh …)` assignment killing the whole run
under `set -eu` with no log line at all — twice, in two different functions, and then a seventh
time on the claim and release path, where a failure could leave work pushed to the remote while
the issue read as never started.

**Still unverified: a reboot.** That a stopped job stays stopped, and that a started one does not
come back, rest on the argument in
[ADR-0004](docs/decisions/0004-job-files-live-on-the-internal-disk.md) rather than on evidence.

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
what this program is allowed to do — including the rule that no file in `runner/lib/` grows past
the point where one person can audit it in a sitting.

## Background

The design rationale, the surveyed alternatives, and the traps found by reading other
orchestrators' source are written up in [docs/decisions/](docs/decisions/) and
[docs/plans/](docs/plans/).
