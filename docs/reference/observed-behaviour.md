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

## Not yet observed

- **The throttled payload of `rate_limit_event`.** Only `{"status":"allowed"}` has ever been seen.
  The probe classifier exists precisely so that nothing depends on the throttled shape. When a real
  throttle is finally observed, record it here verbatim and the matcher can be tightened.
- **A reboot.** That `stop` survives a restart, and that a job left *started* does not come back,
  both follow from the plist living outside `~/Library/LaunchAgents` ([ADR-0003](../decisions/0003-plist-lives-with-the-project.md)).
  Neither has been tested on this machine. Until it is, that guarantee is an argument, not evidence.
