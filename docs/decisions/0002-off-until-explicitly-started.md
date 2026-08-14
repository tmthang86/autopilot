# 2. Autopilot is off until explicitly started

> **Status:** Accepted · **Date:** 2026-08-14

## Context

[ADR-0001](0001-one-task-per-wake-over-persistent-daemon.md) put the operating system's scheduler in
charge of waking the runner, and listed as a benefit that `RunAtLoad` makes the loop resume by
itself after sleep, logout, or reboot — no operator action at all.

The operator rejected that property once it was concrete: autopilot should run when they have
decided to leave the machine on for it, not whenever the machine happens to be on.

That is a different thing from what the plan assumed, and the difference matters more than it first
appears. This runner executes with permission checks disabled. A loop that silently resumes after
every reboot is one that is running at moments nobody chose — during a quick restart to install an
update, on a laptop opened in a meeting, on a machine handed to someone else.

One launchd detail shaped the answer. A plist in `~/Library/LaunchAgents/` is **loaded at every
login regardless of `RunAtLoad`**; that key only controls whether the job fires at the instant it is
loaded. Removing it therefore does not produce "off until started" — it produces "starts up to one
interval later". `launchctl bootout` does not help either, because it lasts only until the next
login. Only `launchctl disable`, which writes to a persistent database, survives a reboot.

## Decision

The job carries no `RunAtLoad`, and the installer writes the plist without loading it. Two commands
control the loop:

- `ctl.sh start <project>` — `launchctl enable` then `launchctl bootstrap`
- `ctl.sh stop <project>` — `launchctl bootout` then `launchctl disable`

`enable` is required in `start` because `bootstrap` refuses a disabled service. After `stop`, the
loop stays off through any number of reboots until `start` is run again.

## Consequences

### Good

- The loop runs only in sessions the operator chose. For a program with permission checks disabled,
  the moments it is live are now a deliberate decision rather than a side effect of the machine
  being powered on.
- Stopping is honest. `stop` means stopped, not stopped-until-Tuesday, which is what `bootout`
  alone would have meant.
- The state is inspectable: `launchctl print-disabled gui/$UID` says whether the loop will come
  back, without reasoning about plist keys.

### Bad

- **The headline property of the original design is gone.** "Leave the machine on and it keeps
  going" now requires starting it once per session. Someone who reboots and forgets will find no
  work done, and nothing will tell them — the loop is silent when it is off, exactly as it is when
  the queue is empty.
- Two more commands to remember, and a state that lives in launchd rather than in the repository,
  so `git status` cannot show it.
- A usage-limit backoff that outlives a reboot now sits in `state.json` waiting for a loop that may
  not be started again for days. Harmless, but it means `resume_after` is no longer a reliable
  signal of when work will actually resume.

### Neutral

- `StartInterval` still fires after wake from sleep, so a machine that sleeps and wakes within one
  started session keeps working without intervention. Only logout and reboot end a session.
- Reversing this is a one-line change to the template plus a new ADR, if leaving it running turns
  out to be what is wanted after living with it.
