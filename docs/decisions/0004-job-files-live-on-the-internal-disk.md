# 4. Job files live on the internal disk, keyed by label

> **Status:** Accepted · **Date:** 2026-08-15 · Supersedes [ADR-0003](0003-plist-lives-with-the-project.md)

## Context

[ADR-0003](0003-plist-lives-with-the-project.md) moved the launchd job file out of
`~/Library/LaunchAgents/`, which launchd auto-loads at login, and put it at
`<project>/.autopilot/launchd.plist`. That solved the auto-load problem and had the pleasant
property of keeping everything the runner owns in one directory.

Installing against the first real target broke it immediately. The project sits on an external
APFS volume, and `mount` shows why:

```
/dev/disk5s1 on /Volumes/REDACTED (apfs, local, nodev, nosuid, journaled, noowners)
```

`noowners` disables ownership on the volume. **launchd refuses to bootstrap a plist from such a
volume**, and says so unhelpfully:

```
Bootstrap failed: 5: Input/output error
Try re-running the command as root for richer errors.
```

External drives are mounted this way routinely, and a project on an external drive is exactly the
case this runner was built for — the target project exists on that volume because it manages
terabytes of downloads.

The failure was made worse by `ctl.sh`, which discarded launchctl's exit status and printed
`autopilot started` regardless. The operator would have walked away believing work was being done
overnight while nothing was loaded at all.

## Decision

Job files are written to `~/.local/share/autopilot/jobs/<label>.plist` — the internal disk, inside
the runner's own installation, in a directory launchd does not auto-scan. The project directory
holds config, state, logs, and the STOP switch, but no plist.

`ctl.sh start` verifies with `launchctl print` that the job is actually loaded, and exits non-zero
with launchctl's own message when it is not.

## Consequences

### Good

- Works regardless of where the project lives — external volume, network mount, anything. The one
  filesystem that has to satisfy launchd is the one the runner is installed on.
- ADR-0003's guarantee is unchanged: the directory is not auto-scanned, so a job left started
  before a reboot still does not come back.
- A start that did not start now says so. Silent failure was the worst property of the old code,
  because it is indistinguishable from a quiet night with an empty queue.

### Bad

- **The runner's files are split across two places.** `.autopilot/` in the project no longer holds
  everything; deleting it leaves the job file behind. `ctl.sh stop` is now the only complete
  uninstall, and nothing enforces running it.
- Job files accumulate in one directory as projects come and go, with nothing pruning them. They
  are inert, but they are litter.
- The label is derived from the project's basename, so two projects with the same directory name
  on one machine would collide. Not handled; it would need the path hashed into the label.

### Neutral

- `AUTOPILOT_PLIST_DIR` still overrides the location, which is how the tests avoid touching
  anything real.
- Nothing prevents a future move back to the project directory for projects on ownership-enabled
  volumes. It would add a conditional for no benefit the operator can see.
