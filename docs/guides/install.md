# Install and run autopilot on a project

## Before you start

You need `git`, `gh` (authenticated with `gh auth login`), `jq`, and the `claude` CLI. The runner
uses nothing else.

The project must be a git repository with a remote, and its work must already exist as issues
carrying the `autopilot` label.

## Install

```sh
sh ~/.local/share/autopilot/install-project.sh /path/to/project [interval-seconds]
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
sh ~/.local/share/autopilot/ctl.sh start  /path/to/project
sh ~/.local/share/autopilot/ctl.sh stop   /path/to/project
sh ~/.local/share/autopilot/ctl.sh status /path/to/project
```

**The loop runs only in a session you started.** It does not come back after a reboot, and `stop`
keeps it stopped across reboots — see [ADR-0002](../decisions/0002-off-until-explicitly-started.md).
The cost of that choice is real: if you restart the machine and forget to start it again, nothing
will be done and nothing will tell you.

Sleeping and waking the machine inside a started session is fine; the next interval fires on wake.

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
