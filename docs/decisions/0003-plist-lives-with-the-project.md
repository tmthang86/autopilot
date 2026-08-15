# 3. The launchd job file lives with the project, not in LaunchAgents

> **Status:** Superseded by [ADR-0004](0004-job-files-live-on-the-internal-disk.md) · **Date:** 2026-08-15

## Context

[ADR-0002](0002-off-until-explicitly-started.md) decided the loop should be off until explicitly
started, and stay off across reboots. The implementation put the job file in
`~/Library/LaunchAgents/` and relied on `launchctl disable` — which persists — to keep a stopped
job stopped.

Preparing to verify that, the gap became obvious. launchd loads **everything** in
`~/Library/LaunchAgents/` at login. `disable` only holds for a job that was explicitly stopped.
So the honest description of the previous implementation was not "off until you start it" but "off
until you start it, unless you reboot without stopping first" — at which point the job is loaded
again and fires at the next interval.

That is precisely the behaviour the operator rejected, surviving in the one case most likely to
occur: a restart during a session that was still running.

`launchctl bootstrap` accepts a plist at any path. Only the standard directories are auto-scanned.

## Decision

The job file is written to `<project>/.autopilot/launchd.plist` and never to
`~/Library/LaunchAgents/`. `ctl.sh start` bootstraps it from there; `ctl.sh stop` boots it out and
disables it. Nothing autopilot writes is ever auto-loaded by launchd.

The file is gitignored: it contains absolute paths from one machine.

## Consequences

### Good

- "Off until started" now holds without qualification, including after a reboot that interrupted a
  running session. There is no ordering the operator has to remember.
- The job file sits next to the config, state, and logs it belongs to. Removing `.autopilot/`
  removes every trace of the runner from that project.
- `disable` becomes belt-and-braces rather than the load-bearing mechanism, so the guarantee no
  longer depends on a persistent database the operator cannot easily inspect.

### Bad

- **A stopped job is now invisible to the usual places people look.** `ls ~/Library/LaunchAgents`
  will not show it, so someone auditing what runs on this machine has to know to look inside each
  project. `ctl.sh status` is the only convenient answer.
- Uninstalling a project by deleting its directory leaves a bootstrapped job pointing at a path
  that no longer exists. `guard_project_present` makes each run a harmless no-op, but the job
  lingers until `bootout`.
- One more path that must agree between `runner/install-project.sh` and `runner/ctl.sh`. They are both wrong
  together or right together, and a test asserts the default location.

### Neutral

- `AUTOPILOT_PLIST_DIR` overrides the location, which is how the tests avoid touching anything real.
- Moving back is a one-line change plus a superseding ADR, if being invisible to a LaunchAgents
  audit turns out to matter more than the guarantee.
