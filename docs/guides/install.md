# Install and run autopilot on a project

## Before you start

You need `git`, `gh` (authenticated with `gh auth login`), `jq`, and the `claude` CLI. The runner
uses nothing else.

The project must be a git repository with an `origin` remote, and the installer
**refuses** a project without one: every `gh` call the runner makes names the
repository explicitly, derived from that remote, so a job installed without it
would stand down at every wake forever. The installer creates the labels the
runner queries; producing the issues themselves is the job of the
`autopilot-deliver` skill.

## Get the runner onto this machine

There is no automatic deployment step yet — copying just `runner/` out to a stable path is
designed (`docs/design/2026-08-15-skill-layer-design.md`) but not built on this branch. Today,
getting the runner onto a machine means cloning this repository there:

```sh
git clone https://github.com/tmthang86/autopilot ~/.local/share/autopilot
```

That puts the **whole repository** at `~/.local/share/autopilot/` — `runner/`, `docs/`, `tests/`,
and `.git` together, not only the part a scheduled run needs. `docs/` and `tests/` sitting there
are harmless; they are simply not pruned yet. Every command in this guide runs a script from
inside `runner/`:

    ~/.local/share/autopilot/runner/install-project.sh
    ~/.local/share/autopilot/runner/ctl.sh

The repository carries Claude Code plugin manifests (`.claude-plugin/`), so `/plugin marketplace
add tmthang86/autopilot` followed by `/plugin install autopilot@autopilot` is the intended route.
It does not work today: `tmthang86/autopilot` is currently **private**, and that command fails for
anyone without access to it. Until the repository is made public — or the skill layer's automatic
deployment lands — cloning it yourself is the only way to get the runner running.

The deployed clone is never edited in place. Fix things in the repository, then `git pull` (or
re-clone) `~/.local/share/autopilot` to pick up the fix. `ctl.sh status` names the version it is
currently running, read from `runner/VERSION`.

## Install

```sh
sh ~/.local/share/autopilot/runner/install-project.sh /path/to/project [interval-seconds]
```

The interval defaults to 2100 seconds (35 minutes). Start conservative: the right number depends on
how fast your tasks burn the usage window, and you cannot know that until you have watched it for a
week.

This creates `.autopilot/config.json`, appends the local files to `.gitignore`, and writes a launchd
job. **It does not start anything.**

## Configure

Four things in `.autopilot/config.json` are project-specific. Everything else can stay as it is.

**`verify`** — the commands that must all pass before anything is committed. This is where "what
language is this project" lives, and it is the only thing standing between the agent and a bad
commit. An empty list is refused rather than treated as success.

```json
"verify": [
  {"name": "format", "cmd": "cargo fmt --check"},
  {"name": "lint",   "cmd": "cargo clippy --all-targets -- -D warnings"},
  {"name": "test",   "cmd": "cargo test"},
  {"name": "types",  "cmd": "pnpm typecheck"}
]
```

**`pacing`** — `daily_task_cap`, `quiet_hours` (e.g. `["09:00-18:00"]` to stay out of your working
day), `circuit_breaker_failures`.

**`autonomy.prepare_only_label`** — issues carrying this label are implemented and committed but
never closed, because their correctness needs a person to look. Defaults to `needs-human`.

**`queue`** — which label means eligible, and which labels exclude an issue.

Commit `config.json`. It is the contract between the project and the runner.

Because it is tracked, **an uncommitted edit to it is reverted** the moment any rejection path runs
`git reset --hard`. Edit, commit, then start.

## Start and stop

```sh
sh ~/.local/share/autopilot/runner/ctl.sh start  /path/to/project
sh ~/.local/share/autopilot/runner/ctl.sh stop   /path/to/project
sh ~/.local/share/autopilot/runner/ctl.sh status /path/to/project
```

`start` exits non-zero if the job did not actually load, and `stop` exits non-zero if the job is
still loaded afterwards. Neither reports something that did not happen. The job file lives on the
internal disk at
`~/.local/share/autopilot/jobs/`, never inside the project — launchd refuses a plist on a volume
mounted `noowners`, which external drives routinely are.

**The loop runs only in a session you started.** It does not come back after a reboot, and `stop`
keeps it stopped across reboots — see [ADR-0002](../decisions/0002-off-until-explicitly-started.md).
The cost of that choice is real: if you restart the machine and forget to start it again, nothing
will be done and nothing will tell you.

Sleeping and waking the machine inside a started session is fine; the next interval fires on wake.

## Upgrading a project installed before 0.1.0

**Read this if a project was installed *and started* with an earlier runner.** The launchd job label
used to be `com.autopilot.<basename-of-directory>`. It is now `com.autopilot.<owner>-<repo>`, derived
from the `origin` remote, because two projects with the same directory name produced one label and
the second install silently overwrote the first's job file.

A job that was already running keeps running under the **old** label. Nothing in the upgrade touches
it, and every command you would use to check has moved on to the new label:

- `ctl.sh stop` boots out the new label, which that job is not, and reports a clean stop.
- `ctl.sh status` looks up the new label and reports `loaded: no`.
- The old job keeps firing every interval, unsupervised, from whatever runner path it was installed
  with.

`stop` now prints a note when it finds nothing loaded under the current label, which is the signal to
do this. Find the orphan:

```sh
launchctl list | grep com.autopilot
ls ~/.local/share/autopilot/jobs/
```

Anything that is not `com.autopilot.<owner>-<repo>` is from the old scheme. Confirm it is really the
one still running, then remove it:

```sh
launchctl print gui/$(id -u)/com.autopilot.<old-label>      # runs, last exit code, the program path
launchctl bootout  gui/$(id -u)/com.autopilot.<old-label>
launchctl disable  gui/$(id -u)/com.autopilot.<old-label>
rm ~/.local/share/autopilot/jobs/com.autopilot.<old-label>.plist
```

Then re-run `install-project.sh` and `ctl.sh start` for the project, and confirm with
`ctl.sh status` that exactly one job is loaded.

## Pause without stopping

```sh
touch /path/to/project/.autopilot/STOP
```

The runner stands down at the next tick and every tick after, until the file is removed. Use this
when you want the job to stay installed but idle. The runner creates this file itself after three
consecutive failures.

## Reading what happened

Everything lands in `.autopilot/logs/`:

- `YYYY-MM-DD.log` — one line per decision, the file to read first
- `run-<issue>-<time>.jsonl` — the raw agent stream for one task
- `launchd.out` / `launchd.err` — what launchd saw

The issue itself is the other half of the record. A completed task carries a closing comment; a
failed one carries the verification output; a blocked one carries the question that stopped it.

## When something looks wrong

**Nothing happens at all.** Check `ctl.sh status`. If it says `loaded: no`, the session was never
started or a reboot ended it. If a `STOP` file is present, remove it.

**The queue looks permanently empty.** launchd jobs inherit no login shell. If `gh` or `jq` is not
on the `PATH` written into the plist, the runner finds nothing and says so quietly. Check
`launchd.err`.

**Work stopped mid-day and the log says the usage window is closed.** That is the design working.
`resume_after` in `state.json` holds the time it will try again; the task that was running was
released untouched and did not consume a retry.

**Three failures in a row.** The circuit breaker created a `STOP` file. Read the last three issues'
comments before removing it — three consecutive failures usually means one broken assumption, not
three unlucky tasks.
