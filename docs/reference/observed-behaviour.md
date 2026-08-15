# Observed behaviour

Everything here was seen directly, with the date. Nothing is inferred from documentation.

This is the highest-priority document in the repository. Seven defects below survived 166 passing
tests and were found only by running against a real repository with the real agent. Each carries a
regression test now; the point of writing them down is that the next person does not pay twice.

---

## 2026-08-14 — first end-to-end run

Target: a throwaway private repository, one issue, agent `sonnet` at `low` effort, verification
`test -f HELLO.md`. Six runs were needed before the loop completed. Each failure is below.

### 1. `gh` resolves the repository from the working directory

**What happened.** Pointed at the throwaway project, the runner reported an empty queue. It was
reading the issue list of the repository the *invoking shell* happened to be sitting in.

**Why it matters far more than a wrong read.** `queue_claim`, `gh issue close`, and
`gh issue comment` would all have acted on that other repository. A run for project A could close
issues in project B.

**Why no test caught it.** Every stub ignored `--repo` entirely, so the argument's absence was
invisible.

**Rule.** Every `gh` invocation names the repository explicitly. The slug is derived once, from the
project's own `origin` remote, by `queue_init`. A project with no origin is refused.

### 2. Command substitution discards the queue cache

**What happened.** The log printed `starting #1:` with an empty title, and the agent received a
prompt containing no task.

**Cause.** The runner does `ISSUE=$(queue_pick)`. Command substitution runs in a subshell, so the
cache `queue_pick` populated vanished when it returned. Every later `queue_field` and
`queue_has_label` then read an empty cache.

**The dangerous half.** `queue_has_label` returning false for everything means a `needs-human`
issue is treated as ordinary work and **closed automatically** — the exact thing the autonomy
ceiling exists to prevent.

**Rule.** Fetching (`queue_load`) is separate from choosing (`queue_pick`). The caller loads in its
own shell, then picks.

### 3. `--add-dir` is variadic and swallows a positional prompt

**Observed error.** `Error: Input must be provided either through stdin or as a prompt argument
when using --print`

**Cause.** `claude --add-dir <directories...>` accepts a list. With `--add-dir .` last, the prompt
passed as a positional argument was consumed as another directory.

**Rule.** The prompt goes in on **stdin**. That is immune to flag ordering, and to a task
description that begins with a dash or contains anything at all. `--add-dir` is also no longer the
final flag, as defence in depth.

### 4. `is_error` appears on tool results, not only on the final result

**What happened.** A run that finished correctly — file created, committed, `"subtype":"success"`,
`"is_error":false` on the final event — was classified as a task failure and thrown away.

**Cause.** The classifier grepped the whole stream for `"is_error":true`. One `tool_result` entry
carried it, because a single shell command inside the session exited 1. That is an ordinary event
inside a successful task.

**Rule.** Only the last `"type":"result"` line decides the outcome.

### 5. `jq`'s `//` treats `false` as absent

**What happened.** The fix for the previous item reported every successful run as an error.

**Cause.** `.is_error // true` returns the right-hand side when the left is `null` **or `false`**.

**Rule.** Test explicitly: `if .is_error == false then "false" else "true" end`. The same trap had
already bitten `state.sh`, where `.field // empty` turned a stored `0` into the caller's default —
the difference between "no backoff" and "unknown".

### 6. The agent commits its own work, so the runner must still push

**What happened.** The loop reported success and closed the issue. The commit was `ahead 1` —
local only. Nothing reached GitHub.

**Cause.** `settle_success` pushed only inside the branch where it had made the commit itself. The
agent is instructed to run the project's checks, and an agent that finished properly has usually
committed already, so that branch was not taken.

**Rule.** Push whatever `HEAD` is, always. If the push fails, the issue is **left open** and the
failure is stated on it. An issue closed while its work sits on one machine reads as delivered and
is not.

### 7. `git reset --hard` with no argument does not undo the agent's commit

**What happened.** Verification failed, the failure was reported, the issue stayed open — and the
rejected commit was still there, with its files in the tree.

**Cause.** `reset --hard` rewinds to `HEAD`, and `HEAD` had already moved to the agent's commit.
Only the uncommitted remainder was discarded.

**Why this is the worst of the seven.** It turns the verification gate off in precisely the case
the gate exists for. Everything downstream still looked correct — the comment, the open issue, the
released claim — so nothing announced that rejected work had been kept.

**Rule.** The runner records `AUTOPILOT_START_SHA` before invoking the agent, and every rejection
path rewinds to exactly that.

---

## Confirmed working, same session

- The full success path: issue claimed, agent run, verification passed, commit pushed, remote and
  local `HEAD` identical, issue closed, `main` untouched.
- The red-verification path: `HEAD` rewound to the starting commit, failure comment posted naming
  the command that failed, issue left open, claim released.
- Model and effort are read from the issue's labels and reach the CLI.
- `.autopilot/` survives both `git clean` and `git reset --hard`.

## Smaller things worth knowing

**GitHub's issue list lags a state change by a second or two.** Reopening an issue and immediately
listing showed an empty queue; the same call a few seconds later returned it. Harmless at a 35
minute interval, but it makes back-to-back manual runs look broken.

**`config.json` is tracked, so `git reset --hard` restores it.** Editing it without committing and
then triggering any rejection path silently reverts the edit. This is correct — the config is the
project's contract — but it wasted a debugging cycle.

**A `claude` session in a project loads that project's `CLAUDE.md` automatically.** The runner does
not need to inject the project's rules into the prompt; the prompt only carries what is specific to
unattended operation.

---

## 2026-08-15 — launchd verification

A second throwaway repository, `StartInterval` 60, plist at `<project>/.autopilot/launchd.plist`.

**The scheduler path works.** With no issues in the repository, a launchd-triggered run logged
`queue bound to …` followed by `queue is empty`, and exited 0.

That short log answers the question that mattered most. **`gh` reaches its keychain token from
inside a launchd job.** launchd gives a child no login shell and its own session context, so the
plausible failure was `gh` being unable to read the credential — which would have presented as a
queue that is permanently empty, indistinguishable from having no work. It does not happen. The
same line proves `git` and `jq` were found through the `PATH` written into the plist; the default
launchd `PATH` is only `/usr/bin:/bin:/usr/sbin:/sbin`, which contains none of them.

**A full task ran under launchd with no manual step.** One issue was added and left alone. The
next interval fired exactly 60 seconds after the previous one, claimed the issue, ran the agent,
passed verification, pushed a commit that matches the remote, and closed the issue. `main` was
untouched. So `claude` is reachable from a launchd job as well — all four dependencies confirmed.

**`stop` works.** After `ctl.sh stop`, `launchctl print` no longer knows the label,
`print-disabled` lists it as `disabled`, and `ctl.sh status` reports `loaded: no`.

**Diagnosing a job that appears not to fire.** `launchctl print gui/$UID/<label>` is the tool.
`run interval = 60 seconds` confirms the plist was parsed; `runs` and `last exit code` say whether
it has executed. A `runs = 0` reading shortly after `bootstrap` means the first interval has not
elapsed yet, not that anything is wrong — `launchctl kickstart gui/$UID/<label>` forces a run
without waiting. `plutil -lint` validates the file separately from whether launchd accepted it.

---

## 2026-08-15 — installing against a project on an external volume

**launchd will not load a plist from a volume mounted `noowners`.** The target project lives on an
external APFS drive, and `mount` reports `noowners` — ownership disabled, which is routine for
external drives. `launchctl bootstrap` answers only:

```
Bootstrap failed: 5: Input/output error
```

Nothing in that message points at the volume. `ls -l` shows sane permissions; `plutil -lint` says
the file is fine. The tell is in `mount`.

Job files now live at `~/.local/share/autopilot/jobs/<label>.plist` on the internal disk
([ADR-0004](../decisions/0004-job-files-live-on-the-internal-disk.md)). Only the runner's own
installation has to satisfy launchd; the project can live anywhere.

**`ctl.sh start` used to report success when the bootstrap had failed**, because it discarded
launchctl's exit status. That is the worst possible presentation of this bug: the operator walks
away believing the loop is running overnight, and an unloaded job looks exactly like a quiet night
with an empty queue. `start` now confirms with `launchctl print` and exits non-zero with
launchctl's own message if the job is not loaded.

## 2026-08-15 — a job that loads, reports success, and never runs

**What happened.** `com.autopilot.myproject` was started, and `ctl.sh start` reported success —
correctly, since the job really was loaded. Hours later, `launchctl list` showed:

```
-	78	com.autopilot.myproject
```

`78` is `EX_CONFIG`. The `-` means no process was running at that moment.

**The runner had never executed once.** No `state.json`, no `launchd.out`, no `launchd.err`, and
`.autopilot/logs/` still empty carrying the mtime of the install. A run that starts and then fails
leaves something behind. This left nothing at all, which places the failure *before* the program.

**Cause — hypothesis, not yet confirmed.** The plist's `StandardOutPath` and `StandardErrorPath`
point into the project's `.autopilot/logs/`, on the external `noowners` volume. launchd opens those
files before executing the program, so a failure there would abort the job with `EX_CONFIG` and
produce no output anywhere — including in the error file it could not open. This matches every
symptom observed, but it has not been proven. Proving it means pointing the log paths at the
internal disk and re-testing.

**Why it slips past the existing defences.** [ADR-0004](../decisions/0004-job-files-live-on-the-internal-disk.md)
moved the *plist* off the external volume, and `ctl.sh start` was hardened to verify the bootstrap
rather than assume it. Both still hold and neither helps: the job genuinely loads, and the label
genuinely exists. `loaded: yes` and `autopilot started` are both true and both useless.

This is the third variant of one failure — *the loop is not doing anything and everything that
reports on it says otherwise*. The first was `start` discarding launchctl's exit status; the second
was a missing label reported as an empty queue; this is a job that loads but cannot run.

**The check that would catch it.** `launchctl print gui/$UID/<label>` reports `runs` and
`last exit code`. A loaded job showing `runs = 0` well after its interval has elapsed, or a
non-zero last exit with no logs on disk, is the signature. `ctl.sh status` reports only whether the
label is known, so it cannot currently distinguish any of this from a quiet night.

## 2026-08-15 — `gh label create` on a label that already exists

**What happened.** Ran directly against a real repository that already had the label, to see the
actual failure shape before writing the installer's error handling:

```
$ gh label create autopilot --repo tmthang86/the companion project --color 0e8a16 --description "Eligible for unattended execution"
label with name "autopilot" already exists; use `--force` to update its color and description
exit=1
```

It exits non-zero, leaves the existing label untouched (no `--force`, no update), and the message
itself names the condition — the string `already exists` is always present when this is the cause.

**Why it matters.** `install-project.sh` creates nine labels on every install and must not fail
just because a project's second install finds them already there. But `gh label create` returns
the same non-zero exit for an expired token, no network, or a repository that does not exist. A
handler that treats *any* non-zero exit as "already exists" reports a clean install over a
repository that has no labels at all — this repository's third time being bitten by the same
shape of failure, where something did not work and the thing reporting on it said otherwise (see
the two 2026-08-15 entries above).

**Rule.** Capture `gh label create`'s stderr and inspect it. Only a message containing
`already exists` is the benign case; anything else is a real failure, gets named verbatim in the
installer's own error output, and fails the install with a non-zero exit — the config, gitignore,
and plist still get written, but the operator is told plainly that labels are missing and why.

## 2026-08-15 — `gh issue list` against a label that does not exist

**What happened.** Measured directly against a real repository, three related calls, clean
readings with no pipeline interference:

| Scenario | exit | stdout | stderr |
|---|---|---|---|
| `gh issue list --label <nonexistent-label>` | 0 | `[]` | empty |
| `gh issue list --repo <nonexistent-repo>` | 1 | empty | `GraphQL: Could not resolve to a Repository with the name 'owner/name'. (repository)` |
| `gh label list --repo <valid-repo> --json name` | 0 | the labels as JSON | — |

A label that was never created is invisible to `gh issue list`: it exits 0 and prints `[]`, exactly
what a queue with nothing ready also prints. `gh issue list` only fails (non-zero exit, a message on
stderr) for a condition it can't paper over, such as a repository it cannot resolve at all.

**Why it matters.** The two conditions this repository actually needs to tell apart — "nothing is
ready" and "the ready label doesn't exist" — produce byte-identical output from `gh issue list`.
Nothing that call returns can distinguish them. Telling them apart requires a second, different
question: `gh label list` names what labels actually exist in the repository.

**Rule.** `queue_candidates` (`lib/queue.sh`) treats a non-zero exit from `gh issue list` as a real
failure and logs its stderr rather than reporting an empty queue. Separately, `queue_load` treats an
empty result — genuinely no work, or a swallowed failure, either way the list is `[]` — as the
trigger to ask `gh label list` whether the configured ready label exists at all, and logs by name if
it does not. A non-empty result is never checked; getting issues back already proves the label
exists.

## 2026-08-15 — a bare `var=$(gh ...)` assignment aborts under `set -eu`, even inside an `if` body

**What happened.** Adding the `gh label list` guard above (in `_queue_check_ready_label`) as a
plain `_names=$(gh ...)` assignment passed all 225 existing assertions. It still crashed
`run-once.sh` silently: `set -eu` treats a failing command substitution assigned to a bare variable
as a statement failure, and that holds even when the assignment sits inside the body of an `if` —
only a failing command that is itself the `if`'s *condition* (or joined with `||`) is exempt.
Reproduced directly:

```
$ sh -c 'set -eu; f(){ if true; then x=$(false); echo "ALIVE"; fi; }; f; echo "returned"'
exit=1          # "ALIVE" never printed, "returned" never printed
```

**Why it matters.** `tests/harness.sh` never sets `-e`, so a test that sources `lib/queue.sh`
directly cannot see this: the same broken function runs to completion in every unit test and only
crashes when `run-once.sh` — which does set `-eu` — calls it for real. The crash left no log line
at all, because the script died before `log_error`'s own `printf` could run in some call orders,
and unconditionally before `run-once.sh` reached its `exit 0` on the empty-queue path. This is the
same shape of failure this repository has now hit five times: something breaks, and the thing that
would report on it says nothing, or says everything is fine.

**Rule.** Every `gh` call inside `lib/` that assigns its output to a variable must be the condition
of an `if`, not a bare assignment — `if _out=$(gh ...); then ... else ...; fi`, matching
`queue_candidates` and `queue_is_closed`'s `2>/dev/null || printf 'UNKNOWN'` fallback, never
`_out=$(gh ...)` on its own line. The same applies one level up: a function that calls such a
guarded helper and wants to keep running regardless of its result must itself guard the call
(`helper || true`) rather than let a non-zero return propagate out of a bare statement. A unit test
against `lib/*.sh` sourced under `tests/harness.sh` cannot catch a `set -eu` regression; only a test
that actually runs `run-once.sh` can.

## 2026-08-15 — the seventh instance: a bare `queue_claim`/`queue_release` statement

**What happened.** Found by the whole-branch review and reproduced against the real `run-once.sh`
with `gh` stubbed to fail only on `issue edit`. `queue_claim` and `queue_release` were each a bare
`gh issue edit … >/dev/null 2>&1`, so the function's exit status *was* `gh`'s and its stderr went to
`/dev/null`; both were then invoked as bare statements under `set -eu`.

| Failing at | exit | last log line | state left behind |
|---|---|---|---|
| claim | 1 | `#5 will run on sonnet at low effort` | agent never invoked, `tasks_today` still 0, nothing on stderr, `launchd.err` empty |
| release, after a green verify | 1 | `verify: t` | **the commit was pushed to origin**, the issue never commented, never closed, still carrying `status:in-progress` |

`status:in-progress` is in `queue.exclude_labels`, so the second row leaves merged work behind an
issue that reads as unstarted and is permanently invisible to the queue. Neither row produced a
single diagnostic line, because gh's stderr was discarded before the shell died.

**Why no test caught it.** `tests/harness.sh` never sets `-e`, so a unit test that sources
`lib/queue.sh` passes whether or not the guard exists. Only a test that executes `run-once.sh` can
see this class — the same lesson as the entry above, one level up the call stack, which is why that
entry's rule now has a regression test that runs the real script rather than the library.

**Rule.** A `gh` call that a caller must be able to react to captures gh's stderr
(`if _out=$(gh … 2>&1 >/dev/null)`), logs it, and returns gh's status. Every caller then guards the
call and decides:

- A **failed claim** stops the run before the agent is invoked, exit 1. The run does not hold the
  issue, and spending an agent turn and a commit on work another wake may already be doing is worse
  than standing down.
- A **failed release, comment, label, or close** happens after the work is pushed and must never
  discard that. It is logged with what was left behind and what it costs, and settlement continues.

Two related contradictions were found at the same time and are recorded here because they are the
same disease — something did not work and everything reporting on it said otherwise:

- `ctl.sh stop` ran `bootout` and `disable` as `2>/dev/null || true` and then printed that autopilot
  was stopped, always. `start` had been hardened for exactly this and `stop` had not. It now verifies
  with `launchctl print` and exits non-zero while the job is still loaded. It cannot see a job left
  under an *older* label — that one is addressed in the install guide's upgrade section, and `stop`
  points at it whenever it finds nothing loaded under the current label.
- `install-project.sh` treated a missing `origin` as benign (`skipping label creation`, plist
  written, exit 0, start banner), while `queue_init` treats it as fatal on every tick, before
  `guard_all`, so not even a STOP file quiets it. A green install that could never run one task. The
  installer now refuses before writing anything.

## Not yet observed

- **The throttled payload of `rate_limit_event`.** Only `{"status":"allowed"}` has ever been seen.
  The probe classifier exists precisely so that nothing depends on the throttled shape. When a real
  throttle is finally observed, record it here verbatim and the matcher can be tightened.
- **A reboot.** That `stop` survives a restart, and that a job left *started* does not come back,
  both follow from the plist living outside `~/Library/LaunchAgents` ([ADR-0003](../decisions/0003-plist-lives-with-the-project.md)).
  Neither has been tested on this machine. Until it is, that guarantee is an argument, not evidence.
