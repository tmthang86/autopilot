---
name: autopilot-review
description: Produce the morning brief for an autopilot project: what the overnight loop did, what is blocked or waiting on a human, and what needs a decision. Use each morning after unattended runs.
---

# autopilot-review

Reads the loop's output and turns it into a short brief, then walks the operator through the
decisions only a person can make. It reports and asks — it never closes a `needs-human` issue,
removes `STOP`, or merges to `main` without being told.

## Sources

Read all six, in the project's `.autopilot/` and git history:

| Source | Answers |
|---|---|
| `.autopilot/logs/<date>.log` | what the loop decided, tick by tick |
| `git log autopilot/main --since` | what landed, and why — commit bodies carry the reasoning |
| open issues labelled `blocked` | the questions the runner stopped to ask |
| open `needs-human` issues with commits | work waiting on a person's eyes |
| `.autopilot/state.json` | `tasks_today`, `consecutive_failures`, `resume_after` |
| `.autopilot/STOP` | whether the circuit breaker tripped |

## The brief

Lead with the facts, then the decisions. One heading per situation below, and skip any that do not
apply. Keep it short: the operator reads this in the morning, not the afternoon.

## Four situations

**The circuit breaker tripped.** Do not offer to remove `STOP` first. Show the last three failure
comments in full; three consecutive failures are usually one broken assumption, not three unlucky
tasks. The operator decides what to fix, then removes `STOP` themselves.

**A `blocked` issue.** Show the question the runner stopped on. The operator answers; the skill then
comments the answer and removes the `blocked` label. This is the only route back into the queue for
a blocked task — do not invent an answer.

**A `needs-human` issue with commits.** State precisely what to open and look at, and what the issue
claims was done. Never close these on the skill's own judgement: correctness here depends on
behaviour a person has to observe.

**Nothing happened at all.** Distinguish the three causes instead of leaving the operator to guess:
never started (session ended, or a reboot ended it), `STOP` present, or `gh`/`jq` absent from the
plist's `PATH` (check `launchd.err`). Say which one it is, with the evidence.

## The missing end of the loop

Nothing merges `autopilot/main` into `main`, and nothing reminds anyone to. The runner is forbidden
from touching `main`, so this is a human act with no prompt attached unless the brief supplies one.
Always end the brief with the merge prompt when there are commits on `autopilot/main`:

```
N commits on autopilot/main, verification green on all of them. Merge them into main?
```

Do not merge without an explicit instruction from the operator.