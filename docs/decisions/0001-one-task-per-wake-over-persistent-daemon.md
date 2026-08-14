# 1. One task per wake, over a persistent daemon

> **Status:** Accepted · **Date:** 2026-08-14

## Context

The runner must keep working through an approved plan while nobody is watching, and must survive
three interruptions that are certain to happen: the machine sleeping or rebooting, the agent's
usage window running out, and a task failing in a way that leaves the working tree dirty.

Three shapes were considered.

**A persistent daemon** holding the queue in memory and dispatching work continuously. This is
what most existing orchestrators do — [Baton](https://github.com/mraza007/baton) runs an async
event loop with a poller, a dispatcher, and a reconciler, defaulting to three concurrent workers.

**In-session scheduling**, using the agent harness's own cron or wakeup facilities. Verified on
2026-08-14: these are documented as in-memory, session-scoped, auto-expiring after seven days,
and firing only while the session is idle. They cannot survive the session, let alone a reboot.
Eliminated on facts, not preference.

**A process that exits after at most one task**, woken by the operating system's scheduler.

## Decision

The runner does at most one task per invocation and then exits. The operating system's scheduler
is the only long-lived component. All state lives on disk and is re-read on every wake.

## Consequences

### Good

- Surviving sleep, reboot, and logout is free rather than engineered. macOS `launchd` with
  `RunAtLoad` fires on login and after wake, so the loop resumes when the machine does, with no
  operator action.
- Usage exhaustion becomes a normal exit rather than an exceptional state. The runner writes a
  timestamp, exits zero, and no-ops on subsequent wakes until it passes.
- Every coordination failure mode reported in Cursor's agent-swarm work — competing planners,
  merge-conflict explosion, contended files, ossification — is a consequence of parallelism, and
  is structurally impossible here. Their measured comparison had one run accumulate over seventy
  thousand merge conflicts; single-track execution cannot produce any.
- Each run starts with an empty context and rebuilds from git. This is cheaper on a subscription
  plan than carrying a session forward, and it removes compaction from the design entirely.
- A crash is bounded. There is no in-memory state to lose, so recovery is whatever the state file
  says.

### Bad

- **Throughput is low by construction** — roughly one task per wake interval. If a project ever
  needs parallel delivery, this design does not extend to it and would have to be replaced.
- **Per-run startup cost is paid every time.** Re-reading rules, plan, and history on each wake is
  wasted work compared to a warm session. Mitigated by keeping the prompt prefix invariant so the
  provider's prompt cache absorbs most of it, but not eliminated.
- **The scheduler is now a platform dependency.** `launchd` is macOS-specific; a Linux host needs
  a systemd timer, which is a second implementation of the same guard logic.
- **No cross-task awareness.** The runner cannot notice that two queued tasks conflict, because it
  never holds both. Dependency ordering has to be declared in the queue rather than inferred.

### Neutral

- Concurrency remains available later without redesign: the guards are already keyed on a lock
  file, so raising the limit is a config change plus worktree isolation. Deliberately not built,
  because the throughput problem it solves has not been observed.
