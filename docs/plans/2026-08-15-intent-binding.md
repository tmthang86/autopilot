# Intent binding — every task names the plan it serves

> **Type:** Plan · **Date:** 2026-08-15 · **Status:** Approved — implemented; the final end-to-end intent-binding verification remains open (see docs/reference/observed-behaviour.md)
> **Scope:** Runner — queue, agent prompt, settlement. No change to guards, verify, or pacing.

## Context

`CLAUDE.md` §2 invariant 4 states that the runner executes intent and never authors it. Today that
is a claim with nothing behind it.

`run-once.sh:70` builds the prompt from five things: issue number, title, body, work branch, project
name. `templates/prompt.tmpl` opens by asserting *"It comes from a plan a human approved"* without
naming which plan, and the agent — which starts every run with an empty context by design — has no
way to reach one. In practice it works from issue prose alone and fills the gaps itself. That is the
exact failure the invariant exists to prevent, and it is currently unguarded.

This plan gives the invariant a mechanism: every task names the plan it serves, the runner resolves
and validates that pointer before spending a single token, and a task that names nothing does not
run.

## What we already know

Verified by reading the code on 2026-08-15, not assumed:

| Fact | Evidence |
|---|---|
| The prompt carries no intent | `run-once.sh:70`, `lib/agent.sh:19-33` — `agent_prompt` takes five arguments, none of them a plan |
| The template asserts approval without naming a source | `templates/prompt.tmpl:5` |
| `milestone` is fetched and never used | `lib/queue.sh:29` selects it; nothing in `run-once.sh` reads it |
| **`depends_pattern` is declared but read by no code** | `templates/config.json:10` declares it; `grep -rn depends_pattern lib/ run-once.sh` returns nothing. `lib/queue.sh:36` hardcodes its own sed expression |
| A blocked settlement path already exists | `lib/settle.sh:92` — `settle_blocked()` |
| Path containment is an already-accepted lesson | Recorded in the consuming project's plan as the one Baton behaviour worth copying verbatim: assert the resolved path stays under the managed directory |
| The agent runs with permission checks disabled | `lib/agent.sh:5-9` maps `bypassPermissions` to `--dangerously-skip-permissions`; flagged by automated security review on 2026-08-15 and accepted by operator decision |
| **The queue already holds eight M0 issues, all labelled `autopilot`** | `gh issue list --repo owner/myproject`, read 2026-08-15 |
| **Those issues already bind intent, but to reference and architecture documents rather than to a plan** | #1 cites `product/prd.md` and `architecture/building-blocks.md`; #3 cites `reference/upstream-api.md` and reproduces its page-clamping table inline; #8 cites PRD requirements by number. None names a file under `docs/plans/` |

The second row changes the design. Intent for a real task is not one plan — it is the plan plus the
requirement, the module boundary, and the recorded API trap. A schema admitting exactly one pointer
would force these eight well-written issues to cite something less specific than what they already
cite, which would make the binding worse in the name of enforcing it.

What is genuinely missing is not information but **enforcement**: nothing today guarantees the next
issue cites anything at all.

Operator decisions taken on 2026-08-15, recorded here because they shape the approach:

- The pointer names the plan, and **the runner does not check any approval marker.** A human writing
  the pointer is the authorisation; re-deriving it from a `Status:` header would encode one
  project's document convention into `lib/`, which `CLAUDE.md` §3 forbids.
- The pointer is **mandatory**. Dropping the approval check does not mean dropping the requirement
  that a task name its source.

## Approach

Three changes to the runner, plus one cleanup.

### 1. Extract the pointer — `lib/queue.sh`

A new `queue_intent()` reads the marker line from the issue body, mirroring how `queue_deps()`
already works. The **marker word** comes from config (`queue.intent_marker`, default `Intent:`); the
expression stays in code.

The line names **one or more** repo-relative paths, space-separated, because that is what the eight
existing issues demonstrate a task actually needs:

```
Intent: docs/plans/2026-08-14-initial-design.md docs/reference/upstream-api.md
```

At least one path is required and every path listed is validated. The marker is `Intent:` rather
than `Plan:` because plans are not the only documents that authorise work — a recorded API trap
constrains an implementation just as bindingly as a plan does.

This deliberately does not put a regex in `config.json`. A sed BRE inside JSON is two escaping
layers and a dialect assumption, and this repository already carries one pattern key that no code
reads. Configuring the word rather than the expression gives a project the naming freedom it might
actually want, and keeps the key honest — it is read on every run.

### 2. Contain the path — `lib/queue.sh`

The pointer arrives as text from a GitHub issue and is consumed by a process running with
permission checks disabled. It is untrusted input and is treated as such. A pointer is accepted only
when all of the following hold:

- it is not absolute
- it contains no `..` component
- after canonicalisation it resolves inside the project root
- it names a file that exists

Anything else is a refusal, not a warning. `Plan: ../../../etc/passwd` is rejected on the same path
as a missing pointer.

### 3. Carry it into the run — `lib/agent.sh`, `templates/prompt.tmpl`

`agent_prompt()` takes the resolved path as a sixth argument. The template gains a constant
instruction to read that file before doing anything else, and to treat it as the boundary of what
the task authorises.

The **path is injected, not the file's contents.** Contents would vary per issue and sit above the
task block, which would break the byte-identical prefix that `lib/agent.sh:17-18` maintains for
prompt-cache reuse. The instruction is constant so it joins the cached prefix; only the path varies,
and it goes in the task block below.

### 4. Refuse early — `run-once.sh`

Resolution happens **before `queue_claim`**, so a malformed task costs nothing. On refusal the
runner calls `settle_blocked()`, comments the reason on the issue, applies `blocked`, and exits 0.

It does **not** consume an attempt and does **not** increment `consecutive_failures`. Retrying will
not fix a missing pointer, and three badly written issues must not trip a circuit breaker meant for
systemic failure.

### Cleanup

`depends_pattern` is either consumed by `queue_deps()` or deleted. A config key that no code reads
is worse than no key, because it tells the next reader that changing it will change behaviour.

**Files changed:** `lib/queue.sh`, `lib/agent.sh`, `run-once.sh`, `templates/prompt.tmpl`,
`templates/config.json`, `tests/test_queue.sh`, `tests/test_agent.sh`, `tests/test_run_once.sh`.

## Work breakdown

| Step | Output | Depends on |
|---|---|---|
| 1 | ADR-0005 — intent binding as a fifth project-specific concern, and why the approval-marker check was dropped | — |
| 2 | `queue_intent()` plus the containment check, with tests covering absolute paths, `..` escape, a symlink pointing outside the root, a missing file, and the happy path | 1 |
| 3 | `agent_prompt()` sixth argument and the template instruction | 1 |
| 4 | `run-once.sh` wiring: resolve before claim, refuse via `settle_blocked`, no attempt consumed, no circuit-breaker increment | 2, 3 |
| 5 | `depends_pattern` made honest — consumed or removed | — |
| 6 | Docs: `CLAUDE.md` §3 four project-specific concerns become five; `docs/guides/install.md` gains the issue format; `templates/config.json` gains `intent_marker` | 2–5 |
| 7 | **Migration, in the consuming project:** add an `Intent:` line to all eight existing M0 issues, citing the documents each one already references in prose. Must land before step 4 is wired | 2 |

## Verification

| Step | How it is proven |
|---|---|
| 2 | Unit tests against a real throwaway repository. Each rejection case asserts the specific refusal, not merely a non-zero exit — a test that passes because the function crashed proves nothing |
| 3 | Two consecutive runs against the same issue; confirm `cache_read_input_tokens` in the second result envelope is non-trivial, proving the added instruction did not break prefix reuse |
| 4 | An issue with no `Plan:` line: confirm `gh` shows the `blocked` label, one comment naming the reason, `status:in-progress` absent, `tasks_today` unchanged, `consecutive_failures` unchanged, and **no `claude` invocation in the log** |
| 4 | An issue whose pointer escapes the root: same outcome, and the log names the containment failure specifically |
| 5 | Change `depends_pattern` in a test config; behaviour changes if consumed, or the key is gone |
| 6 | `shellcheck` clean; full suite run and output read |

**Exercised against a real repository:** one real issue carrying a valid `Plan:` line taken end to
end in the throwaway e2e repository, confirming the agent's transcript shows it read the plan file
before writing code.

## Documentation to update

- [ ] `docs/decisions/0005-intent-binding.md` — new ADR
- [ ] `CLAUDE.md` §3 — the list of project-specific concerns goes from four to five
- [ ] `docs/guides/install.md` — the required issue format
- [ ] `docs/reference/observed-behaviour.md` — only if something is learned the hard way

Consuming projects amend separately: the `.autopilot/config.json` schema gains `intent_marker`, and
the delivery plan's config block changes.

## Risks

| Risk | Level | Mitigation |
|---|---|---|
| The agent is handed a plan path and does not read it, so the binding is decorative | Medium | The instruction sits in the invariant prefix, and end-to-end verification inspects the transcript for the read rather than trusting the instruction |
| A pointer resolves to a file inside the root that is not a plan at all | Medium | Accepted. Containment is a security boundary, not a semantic one — deciding what counts as a plan would be the runner judging intent, which is the thing it must not do |
| Symlink inside the repo pointing outside it defeats a naive prefix check | Medium | Canonicalise before comparing, and test this case explicitly |
| **All eight existing M0 issues become unrunnable the moment this ships** | **High** | Real, not hypothetical — none of them carries an `Intent:` line today. Step 7 adds it to all eight before step 4 is wired, and step 4's verification confirms the queue still selects #2 afterwards. Shipping the refusal before the migration would silently block the entire milestone |
| A project with no plan documents cannot adopt the runner without writing one | Low | Accepted, and recorded in the ADR's consequences. Executing approved intent is the product |

## Out of scope

- **The shard step — turning a plan into issues.** It needs a human signature, since a machine that
  both writes the issue and names its plan authorises itself. Separate plan, Layer A.
- **Checking whether the named plan is approved.** Dropped by operator decision, recorded in ADR-0005.
- **Sandboxing the agent.** The `bypassPermissions` exposure is real and was accepted separately.
  This plan tightens one input path; it does not pretend to close that gap.
