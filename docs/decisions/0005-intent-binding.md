# 5. Intent binding — every task names the plan it serves

> **Status:** Accepted · **Date:** 2026-08-15

## Context

`CLAUDE.md` §2 invariant 4 states that the runner executes intent and never authors it. Until this
ADR that was a claim with nothing behind it: the prompt carried no pointer to any plan, and the
template asserted that the task "comes from a plan a human approved" without naming which plan.
The agent starts every run with an empty context by design, so it had no way to reach one — in
practice it worked from issue prose alone and filled the gaps itself, which is the exact failure
the invariant exists to prevent.

The obvious enforcement — a `Plan:` pointer plus a machine check that the plan is approved — was
considered and rejected. A human writing the pointer is the authorisation; re-deriving approval
from a `Status:` header would encode one project's document convention into `lib/`, which
`CLAUDE.md` §3 forbids. What is genuinely missing is not information but enforcement: nothing
today guarantees the next issue cites anything at all.

## Decision

Every task must name the plan it serves. The runner reads a marker line — `Intent:` by default,
configurable — from the issue body, resolves each path it names, validates that it stays inside
the project root, and refuses to run the task if any check fails. The pointer is **mandatory**.
Dropping the approval-marker check does not mean dropping the requirement that a task name its
source.

The runner does **not** check any approval marker or re-derive whether the plan was approved. A
human writing the pointer is the authorisation; re-deriving it from a convention would encode one
project's document convention into `lib/`, which `CLAUDE.md` §3 forbids.

## Consequences

### Good

- Invariant 4 has a mechanism. A task that names nothing does not run, and a task whose pointer
  escapes the project root is refused as untrusted input — the runner runs with permission checks
  disabled, so the pointer is treated as what it is: text from a GitHub issue.
- Refusal is cheap. Resolution happens before `queue_claim`, so a malformed task costs no tokens
  and no attempt. It does not consume an attempt and does not increment `consecutive_failures`;
  retrying will not fix a missing pointer, and three badly written issues must not trip a circuit
  breaker meant for systemic failure.
- The prompt gains a constant instruction to read the named file before doing anything else.
  Because the instruction is constant it joins the byte-identical prefix that keeps the provider's
  prompt cache warm; only the path varies, and it goes in the task block below (which is not
  cached).

### Bad

- A project with no plan documents cannot adopt the runner without writing one. Accepted: executing
  approved intent is the product.
- **Migration cost is real.** Every existing issue without an `Intent:` line becomes unrunnable the
  moment the refusal ships. Consuming projects must add the line to all live issues before the
  refusal is wired in; this is a manual step with no automation because only a human knows which
  documents authorise a task.

### Neutral

- The marker is `Intent:` rather than `Plan:` because plans are not the only documents that
  authorise work — a recorded API trap constrains an implementation just as bindingly as a plan
  does. The word is configurable (`queue.intent_marker`); the containment rules are not.
- The pointer names one or more repo-relative paths, space-separated. A schema admitting exactly
  one pointer would have forced well-written issues to cite something less specific than what they
  already cite, making the binding worse in the name of enforcing it.