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

The one-liner clones the runner to `~/.local/share/autopilot/` and prepares the project in one
step — it writes the job but starts nothing:

```sh
curl -fsSL https://raw.githubusercontent.com/tmthang86/autopilot/main/install.sh \
  | sh -s -- /path/to/project [interval-seconds]
```

To read the script before running it, download it first:

```sh
curl -fsSL https://raw.githubusercontent.com/tmthang86/autopilot/main/install.sh -o install.sh
sh install.sh /path/to/project
```

If the runner is already present as a git checkout, the script fast-forwards it; a checkout that
has diverged is left untouched and the installer stops. Every command in this guide runs a script
from inside `runner/`:

    ~/.local/share/autopilot/runner/install-project.sh
    ~/.local/share/autopilot/runner/ctl.sh

The repository carries Claude Code plugin manifests (`.claude-plugin/`), so `/plugin marketplace
add tmthang86/autopilot` followed by `/plugin install autopilot@autopilot` succeeds — the
repository has been public since 2026-08-15.

Installing the plugin places the repository in Claude Code's plugin cache, at a path that carries
its version. The plugin ships `autopilot-deliver` and `autopilot-review`, and `runner/deploy.sh`
copies `runner/` out to the stable path that installed launchd jobs point at. Deploying refuses to
overwrite a git checkout, so converting this machine's still-git clone is a deliberate act: push it,
remove the directory, and let the skill deploy a fresh copy.

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

Five things in `.autopilot/config.json` are project-specific. Everything else can stay as it is.

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

**`pipeline`** — `max_rounds` caps the tester↔reviewer loop, `turn_timeout_s` bounds one agent
call, `wake_timeout_s` is the hard ceiling for the whole wake, and `wake_budget_usd` is best-effort
spend. `verify_timeout_s` (default 600) bounds each single `verify` command — a verify command that
outlives it is reported as timed out rather than hanging the wake. `review_lenses` names the final
review calls that always run.

**`autonomy.prepare_only_label`** — issues carrying this label are implemented and committed but
never closed, because their correctness needs a person to look. Defaults to `needs-human`.

**`queue`** — which label means eligible, and which labels exclude an issue. `queue.intent_marker`
defaults to `Intent:` and is the phrase the runner looks for when a task names the documents that
authorise it.

**`intent`** — since ADR-0005, every issue must carry an intent pointer. The marker line reads one
or more repository-relative paths to existing files, space-separated:

```
Intent: docs/plans/2026-08-14-initial-design.md docs/reference/upstream-api.md
```

The runner resolves each pointer before spending a single token. A task without one, or whose
pointer is absolute, escapes the project root, or names a missing file, is refused: the issue is
commented, labelled `blocked`, and costs no attempt and no circuit-breaker increment. Paths are
validated against the project root as untrusted input because the runner executes with permission
checks disabled — a `../` escape or a symlink pointing outside the tree is as unacceptable as an
absolute path.

Commit `config.json`. It is the contract between the project and the runner.

Because it is tracked, **an uncommitted edit to it is reverted** the moment any rejection path runs
`git reset --hard`. Edit, commit, then start.

## The tier ladder and `.autopilot/tiers.local.json`

With the Rust runner, a task's model comes from a **tier** rather than a `model:`/`effort:` label.
Two config layers agree on the ladder:

- `config.json` carries the tier **names** (`tiers`, `roles`, `pipeline`) and is committed.
- `.autopilot/tiers.local.json` carries the machine's **bindings** (`tier → harness, model, effort,
  budget_usd`) and is gitignored, so a second machine re-runs preflight instead of fighting over a
  committed file.

`autopilot-deliver` establishes the ladder with the operator: it runs `autopilot preflight`, proposes
bindings from what this machine can actually reach, the operator confirms, and a tier that does not
resolve stops the flow by name. The proven-adapter table in
[docs/reference/observed-behaviour.md](../reference/observed-behaviour.md) names which harnesses have
actually run against their real CLI.

## The dashboard

The optional read-only dashboard renders the run journals. Build and run it only when a person is
present:

```sh
cd dashboard && go build -o /tmp/apdash .
/tmp/apdash --addr 127.0.0.1:8787
```

It is never invoked by the runner, binds `127.0.0.1` by default, and its failure cannot affect a
scheduled run. The same facts — including an orphaned role — are available from
`autopilot status`, so the dashboard is never the only route.

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

## After renaming or transferring the GitHub repository

The pre-0.1.0 migration above is not the only way a job outlives its label. `com.autopilot.<owner>-<repo>`
is derived from `origin` at install time, and nothing re-derives it afterwards. Renaming the repository,
or transferring it to a different owner, changes what `origin` *should* resolve to — but the label
already on disk, and the one the running job was bootstrapped under, does not change with it. This
happens at a time nobody is thinking about autopilot: the rename is a GitHub action, not an autopilot
command.

The symptoms are the same as the pre-0.1.0 case, because the underlying problem is the same — the
installed label no longer matches `label_for_project` for this project:

- `ctl.sh stop` boots out the label the current `origin` derives, which the running job is not, and
  reports a clean stop.
- `ctl.sh status` looks up the current label and reports `loaded: no`.
- The old job keeps firing every interval, unsupervised, against whatever repository slug it was
  started with — which, after a rename, GitHub still resolves via redirect, so it may keep working
  silently instead of failing loudly.

Find and remove it the same way:

```sh
launchctl list | grep com.autopilot
ls ~/.local/share/autopilot/jobs/
```

Anything that is not `com.autopilot.<current-owner>-<current-repo>` is a candidate. Confirm it is
really the one still running, then remove it:

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
