# Multi-harness delivery with a role pipeline — design

> **Type:** Design · **Date:** 2026-08-16 · **Status:** Approved
> **Scope:** Six changes taken together: a harness abstraction, a project-defined tier ladder, a
> three-role runtime pipeline, provider preflight in the deliver skill, a run journal with a separate
> dashboard, and a third skill — `autopilot-planning` — that produces the plans the other two consume.
> Includes the decision to rewrite the runner in Rust and what replaces Rule Zero.

## Context

The runner works. `tests/run.sh` reports `ALL PASS` across fifteen files, `shellcheck` is clean, and
the loop has been exercised end to end against real repositories under launchd. What it does not do
is vary: every task is implemented, tested, and reviewed by a single invocation of one hard-coded
CLI, `claude`, at a model chosen by an issue label.

A review conducted before this design also found five declared-but-unread configuration keys. They
are listed below because three of them stop being cosmetic the moment this design lands.

### What the review found

| Key or behaviour | State | Why this design cannot defer it |
|---|---|---|
| `agent.turn_timeout_s` | declared in `templates/config.json`, read by nothing | A task becomes 3×N agent calls. A local model that hangs holds the lock forever, and `guard_lock` then blocks every later wake |
| `pacing.max_attempts_per_issue` | declared, read by nothing | The pause-and-resume path below depends entirely on a per-issue attempt budget. `settle_failure` currently releases the claim with no counter, so only the global circuit breaker limits retries |
| `autonomy.default` | declared, read by nothing | Only `prepare_only_label` is honoured; the "classes of task" §3 describes is one class |
| cumulative spend ceiling | `CLAUDE.md` §3 names it; only per-invocation `agent.max_budget_usd` exists | One wake can now spend 3×N times what one call spends |
| `ctl.sh status` | reports the label is known; does not read `launchctl print` runs/last-exit, and does not compare `VERSION` against the plugin | `observed-behaviour.md` names the first as *"the check that would catch it"* for the `EX_CONFIG` failure; the skill-layer design requires the second |

Two things recorded as unverified on 2026-08-15 remain unverified: a reboot, and intent binding taken
end to end against the real agent.

## Decisions taken

Each was an operator decision made on 2026-08-16, not an inference.

| # | Decision | Consequence |
|---|---|---|
| 1 | Scope is all five: harness abstraction, tier ladder, role pipeline, preflight, journal + dashboard | One spec, one plan sequence. The five are not independently shippable — the pipeline needs the ladder, the ladder needs the abstraction |
| 2 | The coordinator / planner / classifier is **the agent running the deliver skill**, not a runtime role | Escalation is a pause for a human-present session, not a fourth unattended agent. ADR-0005 and §2.4 survive unchanged |
| 3 | Runtime is three roles: implement (T), test (T, clean context), review (T+1, blocking gate) | One task is at least three agent invocations |
| 4 | A tester rejection goes to the reviewer, who **adjudicates and fixes**, then hands back to the tester | The higher tier is the one that repairs the lower tier's work |
| 5 | The tester↔reviewer loop is capped at **2 rounds**; beyond that the task escalates to the coordinator and **pauses** | Pause must preserve work — see *The branch model* |
| 6 | A reviewer that has already fixed code does **not** also gate that code; the tester's pass is the gate | Closes the self-approval hole opened by decision 4 |
| 7 | Paused work lives on `autopilot/task-<n>`, pushed to the remote | Survives `git reset --hard`, survives losing the machine |
| 8 | Provider configuration has two axes, **harness × model** | `ollama` and `lmstudio` are values of *model*, reached through a harness. They are not harnesses: they expose no tools and cannot edit a file, run a command, or commit |
| 9 | The tier ladder is an **ordered array the project names itself**; `tier_offset` resolves "one step up" | No tier name is known to the runner. §3 portability holds |
| 10 | Tier **names** are committed; tier **bindings** are machine-local and gitignored | A project cloned to a second machine re-runs preflight instead of fighting over a committed file |
| 11 | `usage_limit` generalises to **`provider_unavailable`** | Covers a closed usage window, a dead local endpoint, and an expired token. All three back off without consuming the retry budget |
| 12 | Role verdicts are written to **files**, never parsed from stdout | Harness-agnostic. A missing or malformed verdict is a `task_failure`, never a pass |
| 13 | `verify` runs at **every loop boundary**, not only at the end | It costs no tokens, so it must never be the last thing to discover the build is broken |
| 14 | The runner is **rewritten in Rust**, shelling out to `git` and `gh` exactly as today | Supersedes Rule Zero's "POSIX shell only". See *Why Rust* |
| 15 | The rewrite is a **single cutover**; the shell runner is frozen as the reference specification | Two sources of truth are the thing §1 exists to prevent, so the overlap is bounded and ends |
| 16 | The dashboard is **Go + htmx**, living outside the runner | Server-rendered fragments, htmx vendored as one embedded file, no build step beyond `go build` |
| 17 | A third skill, **`autopilot-planning`**, precedes delivery. The trio is planning → deliver → review | The product becomes idea-to-shipped, not queue-to-shipped |
| 18 | Planning writes everything and **commits nothing**; the operator commits | Deliver's commit gate is a mechanical check. A skill that commits its own plan deletes the gate's meaning |
| 19 | The documentation set is **arc42 + Diátaxis + Nygard ADRs**, as already proven in a companion project | Not invented here. Adopted from a repository where it has been running |
| 20 | **`AGENTS.md` is the single source of agent rules.** `CLAUDE.md` and every other tool's rule file are thin pointers to it | Copying rules into five files is the stale-document disease the sync table exists to prevent |
| 21 | Plan steps are **structured**: goal · done-when · verify · Intent · depends-on · tier · needs-human | Sharding becomes mechanical, and a malformed step is caught while writing the plan rather than at 3 a.m. |
| 22 | Tracking extends that project's existing artifacts rather than adding a `STATUS.md` | One file every commit touches is a merge conflict per task branch |
| 23 | The **implementer writes the documentation its step binds**; the reviewer treats a missing update as grounds for rejection | The §4 sync table becomes enforceable with nobody watching |
| 24 | The reviewer runs a **configured list of lenses, each a separate fresh call** | Independence is the mechanism, so one prompt with three headings is not a substitute. Cost is the trade, and the list is config |
| 25 | Cost is accounted **per role and per tier**, never per wake, and every figure carries its `cost_source` | The measured asymmetry: cheap roles make the tokens, the dear role makes the bill. A wake total hides the only actionable number |

## Why Rust, and what replaces Rule Zero

Rule Zero states the constraint as *POSIX shell only*, but the reason it gives is different:
**anything the operator must trust and cannot read is a liability**, and the codebase must be
readable in one sitting. Shell was the means; auditability was the end.

`docs/reference/observed-behaviour.md` is the argument for changing the means. Of the defects that
survived 166 passing tests, most are shell semantics rather than logic:

| Defect | Class |
|---|---|
| `x=$(false)` under `set -eu` kills the script even inside an `if` body — twice, at two levels of the call stack | shell semantics |
| `jq`'s `//` treats `false` as absent — twice, in `state.sh` and `agent.sh` | weak typing across a process boundary |
| Command substitution runs a subshell, discarding the queue cache | shell semantics |
| `git reset --hard` with no argument rewinds to a `HEAD` that has already moved | not shell; a genuine logic defect |

Three of those four classes are unrepresentable in Rust: `Result` must be handled, `Option<bool>`
distinguishes `false` from absent, and no construct silently discards state on return. The fourth
would have been caught by neither language.

The design in this document — three roles, bounded rounds, five harness adapters, a journal, per-call
timeouts, cumulative wake ceilings, and a branch lifecycle — is past the size where shell is the
honest choice.

**The constraint that replaces Rule Zero's first line:**

| Old | New |
|---|---|
| POSIX shell only | Rust, with a dependency tree an operator can enumerate. `git` and `gh` remain child processes |
| No runtime dependencies beyond `git`, `gh`, `jq`, and the agent CLI | Same list minus `jq` — parsing moves in-process. Nothing else is added |
| No file in `runner/lib/` exceeds ~150 lines | No module exceeds ~200 lines |
| No network calls except through `gh` and the agent CLI | Unchanged, and now checkable by grepping for HTTP crates rather than by reading every line |

**Why `git` and `gh` stay as child processes.** Every rule in `observed-behaviour.md` is a finding
about how those two programs *behave* — `gh` resolving the repository from the working directory,
`gh label create` exiting non-zero for both a duplicate and an expired token, `gh issue list` exiting
0 with `[]` for a label that does not exist. Those findings are about the CLI. Replacing it with
`octocrab` discards every one of them and requires re-measuring the same ground on a different
surface. `gh` also already solves keychain access under launchd, which is confirmed working
(2026-08-15) and is exactly the kind of thing that fails quietly.

## Architecture

```
autopilot/                       the plugin repository
├── runner/                      Rust. Compiled to one binary, deployed to ~/.local/share/autopilot/
│   ├── src/
│   │   ├── main.rs              argument parsing, one task per invocation, then exit
│   │   ├── config.rs            two-layer config load and validation
│   │   ├── tier.rs              ladder resolution, tier_offset
│   │   ├── queue.rs             gh: candidates, claim, release, labels, intent
│   │   ├── guard.rs             lock, branch, quiet hours, daily cap, STOP, tiers
│   │   ├── pipeline.rs          the three-role loop and its exit paths
│   │   ├── verify.rs            the gate
│   │   ├── settle.rs            git and gh writes; the only module that writes history
│   │   ├── journal.rs           append-only JSONL
│   │   └── harness/             ONE FILE PER HARNESS. No harness name appears outside here
│   │       ├── mod.rs           the trait, and the default classifier
│   │       ├── claude.rs  opencode.rs  pi.rs  codex.rs
│   └── templates/               prompt templates, one per role
├── skills/                      autopilot-planning · autopilot-deliver · autopilot-review
│                                the trio: idea → plan → issues → unattended delivery → morning brief
└── dashboard/                   Go + htmx. Read-only. The runner never invokes it
    ├── main.go                  journal reader, HTTP handlers, embedded assets
    └── ui/                      templates + vendored htmx.min.js
```

### The boundary, restated and widened

The skill-layer design established one sentence: **skills run when a person is present; shell runs
when nobody is.** This design adds a second consumer and keeps the same shape:

```
runner  ──writes──▶  journal (.autopilot/runs/**/journal.jsonl)  ◀──reads──  dashboard
```

The runner knows nothing about the dashboard, and the dashboard never writes. Both halves are
grep-checkable, and both become tests:

```
runner: no reference to  skills/  or  dashboard/       (permanently empty)
outside runner/src/harness/: no occurrence of  claude | opencode | pi | codex
```

### The harness adapter contract

A harness is an agentic CLI: it has tools, it can read and edit files, run commands, and commit. A
model server is not a harness. Each adapter implements one trait, and nothing else in the codebase
knows a harness exists.

| Method | Returns | Called by |
|---|---|---|
| `available()` | binary present **and** authenticated | preflight (deliver) and `guard_tiers` (every wake) |
| `models()` | model identifiers actually reachable | preflight, to verify a named model exists |
| `run(prompt, cwd, log, params)` | spawns it, prompt on **stdin**, honouring `turn_timeout_s` | the pipeline |
| `probe()` | one trivial call — is the provider reachable at all? | classification |
| `classify(log, status)` | `Ok` \| `TaskFailure` \| `ProviderUnavailable` | the pipeline |

`classify` has a **shared default** in `harness/mod.rs`: exit status 0 is `Ok`; otherwise `probe()`
decides between `TaskFailure` and `ProviderUnavailable`. Only `claude.rs` overrides it, to keep the
two refinements that were paid for in `observed-behaviour.md` §4 and §5 — that only the final
`type:result` event decides, and that `is_error` must be compared against `false` explicitly rather
than defaulted. A new adapter is therefore correct from its first line and merely less precise.

**The prompt always goes in on stdin.** This is not a Claude-specific workaround; a task description
is arbitrary text that may begin with a dash, and stdin is immune to that and to flag ordering for
every harness.

### Measured harness behaviour, 2026-08-16

Recorded here because `CLAUDE.md` §4 forbids inventing an output shape. Everything in the *measured*
rows was run directly on the operator's machine on 2026-08-16.

| | claude | pi | opencode | codex |
|---|---|---|---|---|
| Spawn | `-p --output-format stream-json --verbose --model M --effort E --max-budget-usd N`, prompt on stdin | `-p --mode json --provider P --model M --thinking L` | `run --format json -m provider/model --variant V --auto` | not installed |
| List models | — | `--list-models` | `models [provider] --verbose` | — |
| Auth check | `--version` plus probe | `auth check --provider P --json` | `providers`; errors out if config invalid | — |
| Skip permissions | `--dangerously-skip-permissions` | `--tools` / `--exclude-tools` | `--auto` | — |
| Cost | **measured:** `total_cost_usd` and `modelUsage[model].costUSD` on the final `result` event | **not measured** | `stats --days N`, `export <session>`; **not measured** | — |

**Two traps measured directly, and one machine-state finding:**

1. **`pi auth check` exits 0 regardless of readiness.** Status is in the JSON, not the exit code:
   ```
   {"status":"not_ready","provider":"anthropic","reason":"credentials_not_configured"}
   {"status":"ready","provider":"deepseek","authType":"api_key"}
   {"status":"not_ready","provider":"ollama","reason":"provider_not_found"}
   ```
   Reading the exit status reports a provider with no credentials as ready — the exact failure shape
   this repository has now paid for seven times. It also confirms **`pi` does not support `ollama` or
   `lmstudio` natively**; both answer `provider_not_found`, while nine hosted providers answer
   `credentials_not_configured`.

2. **`opencode models` lists what is *reachable*, not a catalogue.** Good for preflight, but it is
   silent about providers that are merely unconfigured.

3. **On this machine, only `claude` is currently usable.** `opencode` fails every command because
   `~/.config/opencode/config.json` is invalid (`mcp.gitnexus` is missing the `enabled` key); `pi`
   has credentials for `deepseek` only; the LM Studio server is off and `lms ls` times out waiting
   for its daemon; `ollama` and `codex` are not installed. The design must tolerate this, and the
   plan must not claim support for an adapter that has never been run.

**Adapter order, and what "supported" means.** `claude` first (it exists), then `opencode` (it
unlocks both local models and its free gateway models), then `pi` (deepseek plus nine hosted
providers), then `codex` when it is installed. An adapter that has never executed against its real
CLI ships as *unproven* and is named as such in the guide — a recorded fixture that was never
observed is an invented JSON shape by another name.

### The tier ladder, in two layers

Layer 1, `.autopilot/config.json` — committed, the project's contract:

```json
{
  "tiers":    ["light", "standard", "deep"],
  "roles":    { "context_docs": ["docs/plans/2026-08-16-overall.md", "CLAUDE.md"],
                "implement": {"tier_offset": 0},
                "test":      {"tier_offset": 0},
                "review":    {"tier_offset": 1} },
  "pipeline": { "max_rounds": 2, "turn_timeout_s": 2700,
                "wake_timeout_s": 10800, "wake_budget_usd": 25.0,
                "review_lenses": ["plan-conformance", "partial-failure",
                                  "documentation-truth"] },
  "agent":    { "permission_mode": "bypassPermissions", "default_tier": "standard" },
  "queue":    { "tier_label_prefix": "tier:", "...": "unchanged" }
}
```

Layer 2, `.autopilot/tiers.local.json` — gitignored, this machine's bindings:

```json
{ "light":    {"harness": "opencode", "model": "opencode/nemotron-3.5-lightning-free", "effort": "low",  "budget_usd": 0},
  "standard": {"harness": "claude",   "model": "sonnet", "effort": "low",  "budget_usd": 5.0},
  "deep":     {"harness": "claude",   "model": "opus",   "effort": "high", "budget_usd": 15.0} }
```

`tier_offset: 1` applied to the top tier resolves to itself: the ladder has a ceiling and does not
overflow. Config load fails, loudly and by name, if any tier in layer 1 has no binding in layer 2 —
a configuration error, not a task failure.

`agent.default_model`, `agent.default_effort`, and `agent.max_budget_usd` are removed, and the
`model:` / `effort:` labels are replaced by `tier:<name>`. An old config therefore fails to load
rather than running with silently wrong parameters. Migration is the deliver skill's job.

### Why three roles at all

Everything below follows from one claim, so it is worth stating precisely rather than assuming:

> **An agent that implements, tests, and reviews its own work is checking that work with the same
> context that produced it.** The check and the thing being checked share an origin, so they share
> the same mistakes.

That is the whole argument. What follows is why it holds, what it costs, and what shape it forces.

#### Three distinct mechanisms, not one

They are worth separating because each is addressed differently, and conflating them produces a
design that pays for one and believes it bought all three.

**1. Anchoring.** By the time an implementation is finished, the session's context holds the
reasoning that produced it, the assumptions it rested on, and every dead end taken along the way.
An agent asked to write tests at that point writes tests for **what it built**, not for what was
asked. The tests then encode the implementation's assumptions rather than the requirement's, and
they pass — which is the failure mode, not the success.

This is not theoretical. The companion project records two instances found by review, both of which
survived a green suite:

- *"A test that runs without asserting. Nine `it()` blocks once reported 8/8 passing while checking
  nothing."*
- *"A fixture invented rather than recorded… invented fixtures are what hid the `current.size` bug
  while every unit test stayed green."*

Neither is a capability failure. A stronger model in the same session writes the same test, because
the test looks correct from inside the assumptions that produced it.

**2. Attention decay across a long context.** The material that matters most — the plan, the intent
documents — is read first, and is therefore furthest away by the time judgment is needed. A long
implementation session pushes it behind thousands of tokens of tool output. A fresh session reads
those same documents with them **adjacent to the judgment they inform**. The document has not
changed; its distance from the decision has.

**3. Self-assessment is not review.** The companion project states it flatly: *"A step marked done
by whoever did it is a self-assessment, not a review."* That repository has already produced the
failure in the opposite direction — work finished on disk while the status table still read `⏳`. A
status that is wrong in either direction is the same defect.

#### What each role is actually buying

This is why the tiers are assigned the way they are, and the asymmetry is deliberate.

| Role | Tier | What it contributes | What it does **not** contribute |
|---|---|---|---|
| **Implementer** | T | The work | — |
| **Tester** | T — *the same* | **A clean context.** It has never seen how the code came to be, so it tests the requirement rather than the implementation | More capability. That is not what is missing |
| **Reviewer** | T+1 — *higher* | **More capability**, for adjudication and repair | A clean context alone. By the time it is called, a disagreement already exists |

The reviewer does not read the work once and form a general opinion. It runs a **list of lenses**,
each a separate fresh call seeing only its own question — the pattern Cursor reports as *"stacked
review lenses: multiple independent reviewers catch errors that individual reviewers miss"*, and the
same shape as the two-lens plan review `autopilot-planning` performs before approval.

`pipeline.review_lenses` is an ordered list in `config.json`, and each entry is one invocation:

| Lens | The single question it is given |
|---|---|
| `plan-conformance` | Does this implement the approved task, **and only that**? Where has scope widened without being asked for? |
| `partial-failure` | What happens on a crash, a restart, a half-written file, or a command that exits non-zero midway? What is asserted rather than checked? |
| `documentation-truth` | For every row the sync table binds to this change, does the document now tell the truth? (decision 23) |

A lens whose finding cannot be evidenced is a rejection, not a remark.

**Why separate calls rather than one prompt with three headings.** The anchoring argument does not
stop at the session boundary — a single reviewer that has just argued the code conforms to the plan
is no longer a neutral reader of whether it survives a restart. One call with a checklist is
cheaper and weaker, and saying so is better than implying the two are equivalent.

**What it costs, and the escape.** Each lens is one call at the dearest tier. A project that cannot
afford three lists one, and gets a reviewer no worse than a single general review — it simply gives
up precisely what was measured to help. The list is config for that reason.

**The tester is the same tier on purpose.** Its advantage is *ignorance of the implementation's
history*, not power. Paying for a stronger model to do a job whose entire value comes from a clean
context is spending on the wrong axis — it buys nothing the cheap model in a fresh session does not
already have.

**The reviewer is higher on purpose.** Its job is a different kind of problem: deciding which of two
disagreeing parties is right, and then repairing the code. That is a capability question, not a
context one. Decision 23 adds a second reason — judging whether a document still tells the truth
about the code is harder than writing the code.

#### Independent corroboration, with numbers

The argument above was reached from this repository's own failures. Cursor's
[agent swarm and model economics](https://cursor.com/blog/agent-swarm-model-economics) report
reaches the same structural conclusion from a different direction and, unlike this document,
attaches measurements to it:

> *"a planner never implements, so its context never fills with low-level detail, and a worker never
> plans, so it can spend all its context on one narrow piece of work"*

Three of their figures change how this design should be read:

| Measured | What it means here |
|---|---|
| **$1,339 to $10,565** for the same task complexity, at **comparable quality** | An eight-fold cost spread decided purely by which model does which job. This is the strongest available argument that the tier ladder must exist, and it is stronger than any reasoning in this document |
| Workers consumed **69–90% of all tokens**, while the dearer planner consumed roughly **two-thirds of the dollars** | Cost does not follow token volume. The expensive role produces little and dominates spend — so cost must be accounted **per role and per tier**, never per wake |
| A full cheap worker fleet cost **$411** beside a frontier planner | The low tier is cheaper than intuition suggests. An implementer tier can be set far below the reviewer's without the usual hesitation |
| *"few moments in a large task genuinely require frontier intelligence"* — decomposition and design decisions | In this design those moments are exactly two: the coordinator, and the reviewer while adjudicating. Everything else may be cheap. This is independent support for decision 2 |

One convergence is worth noting on its own: they prevent a planner contradicting itself with
**compile-checked pointers to design documents**. That is the same mechanism as `Intent:`,
`queue_intent`, and `validate-plan.sh` — a reference that is verified to resolve rather than trusted
to. Two systems arrived at it separately, which is the best evidence available that it is load
bearing rather than stylistic.

#### What this design deliberately does not take from that work

Named here because each is attractive, and an attractive idea rejected without a reason gets
adopted later by someone who does not know it was considered.

- **Massively parallel agents with merge agents to resolve collisions.** ADR-0001 chose one task per
  wake. Their own numbers argue *for* that choice at this scale: the hottest file in their older
  harness saw **7,771 conflicts across 1,173 agents**, and the coordination apparatus that fixes it
  is larger than this entire runner. Serial delivery has no collisions to arbitrate.
- **A planner that is an agent.** Theirs decomposes goals and makes design decisions autonomously.
  §2.4 forbids exactly that: the runner executes intent and never authors it. Decision 2 puts the
  coordinator with a person instead. **This is the real divergence between the two designs**, and it
  is a deliberate trade — less autonomy, in exchange for never waking up to work nobody approved.
- **"Licensed breakage"** — an agent permitted to change core code provided it leaves an explanatory
  comment that propagates downstream. It is an agent granting itself scope, which is the one thing
  the blocked-and-ask path exists to prevent.

#### The ladder of checks

The four checks are not four amounts of the same thing. Each catches a class the one below it
structurally cannot, and each costs more:

| Check | Kind | Cost | Catches |
|---|---|---|---|
| `verify` commands | Mechanical, deterministic | **No tokens** | Anything the project already asserts about itself |
| **Tester** | Judgment, clean context, same capability | One call | What the commands never asserted — the untested path, the wrong requirement |
| **Reviewer** | Judgment, more capability | One call, dearer | What the tester's judgment missed; then repairs it |
| **Coordinator** | Authority, human-present | A person's morning | What none of them may decide: whether disputed work should land at all |

Reading it downward explains decision 13. `verify` costs nothing, so it runs at **every** boundary —
it must never be the last thing to discover the build is broken, because by then the two dearest
checks have already been paid for.

Reading it upward explains the escalation. Each layer hands upward only what it cannot settle, so
cost is incurred only where disagreement actually exists.

#### The workflow

```
  ┌──────────────────────────────────────────────────────────────────┐
  │ COORDINATOR — a person is present                                │
  │ autopilot-planning · autopilot-deliver · autopilot-review        │
  │ authors intent · classifies the task · assigns the tier          │
  │ decides what the machine may not: close, requeue, or abandon     │
  └──────────────────────────────────────────────────────────────────┘
      │                                            ▲
      │ issues: goal · Intent: paths · tier        │ ESCALATE — pause; the work
      │                                            │ stays on autopilot/task-N
      ▼                                            │ and the argument on the issue
  ═══════════════ ONE WAKE · NOBODY IS WATCHING ═══╧═══════════════════

  Every role starts a FRESH session and reads the documents itself: the
  project context docs, plus the task's Intent: paths. The brief names
  paths; it never pastes excerpts, and no transcript is handed on.

  ┌────────────────┐
  │ IMPLEMENTER  T │  writes code, tests, and the docs its step binds
  └───────┬────────┘
          ▼
  ┌────────────────┐  costs no tokens · runs at every boundary
  │    verify()    │──── red ────────────────┐
  └───────┬────────┘                         │
       green                                 ▼
          ▼                       ┌─────────────────────┐
  ┌────────────────┐   reject     │ REVIEWER        T+1 │
  │ TESTER       T │─────────────▶│ more power          │
  │ same power     │              │ adjudicates AND     │
  │ clean context  │◀─ hands back─│ repairs             │
  │ outside voice  │              └──────────┬──────────┘
  └───────┬────────┘                         │
       pass                        after 2 rounds ──▶ ESCALATE
          ▼
  ┌────────────────┐  the gate — but ONLY if the reviewer has not already
  │ REVIEWER   T+1 │  repaired this task. It never grades its own fix;
  │ one call PER   │  there, the tester's pass is the gate.
  │ LENS, each a   │    plan-conformance · partial-failure · documentation-truth
  │ fresh session  │  Independence is the mechanism: one call with three
  └───────┬────────┘  headings is cheaper and weaker, not equivalent.
          ├──── any lens rejects ──▶ ESCALATE, naming the lens
       approve
          ▼
  ┌────────────────┐  UNCONDITIONAL — §2.1 has no bypass
  │    verify()    │──── red ──▶ reset to START_SHA · attempt++
  └───────┬────────┘             attempts exhausted ──▶ ESCALATE
       green
          ▼
  merge --ff-only ▶ autopilot/main ▶ push ▶ close issue ▶ delete branch
  ════════════════════════════════════════════════════════════════════
```

Three properties of that picture are the design, and each is load-bearing:

- **Every arrow crossing into a role crosses a context boundary.** No session is resumed, no
  transcript is handed on. What passes between roles is the *code itself*, the verdict files, and
  the paths each role reads for itself. This is the mechanism, not an implementation detail: a
  handed-on transcript would reintroduce exactly the anchoring the split exists to remove.
- **The escalation arrow only ever points up.** A role that cannot settle something hands it to
  something with more capability, and the top of that ladder is a person — never a fourth
  unattended agent, because deciding whether disputed work should land is authoring intent, which
  §2.4 reserves for a person.
- **The reviewer appears twice and is never allowed to grade its own repair.** Where it has already
  intervened, the tester's pass is the gate. Decision 4 created that hole; decision 6 closes it.

#### What this costs, stated plainly

Writing `L` for the number of review lenses and `R` for `max_rounds`:

| | Calls | With the defaults `R=2`, `L=3` |
|---|---|---|
| Agreement on the first pass | `2 + L` | **5** |
| Disagreement, converging on the last allowed round | `2 + 2R + L` | **9** |
| Disagreement, escalating | `2 + 2R` | **6**, then a person |

Against one call today. That is the price, and it is not small.

Three things bound it, and one does not:

- **The tier ladder does most of the work.** Cursor measured an eight-fold spread at comparable
  quality; nine calls on a well-chosen ladder can cost less than five on a badly chosen one. The
  number of calls is the wrong thing to optimise first.
- `verify` runs at every boundary and costs nothing, so broken work never reaches a paid check.
- `wake_timeout_s` is a hard ceiling and is enforceable on every harness.
- `wake_budget_usd` is **not** a reliable bound: only `claude` reports cost today. It is
  best-effort, and saying otherwise would be the kind of false assurance this repository has already
  paid for seven times.

**Cost is accounted per role and per tier, never per wake.** This follows directly from the measured
asymmetry above: the cheap roles produce most of the tokens and the dear one produces most of the
bill, so a single wake total hides the only number that can be acted on. The journal records
`role`, `tier`, `harness`, `model`, and `cost_usd` on every `role_end` for exactly this reason, and
the dashboard aggregates by tier first.

#### Why not more roles, or fewer

**Fewer** — a reviewer alone, with no tester — was rejected because it leaves the anchoring problem
untouched. The reviewer is expensive and is invoked once; the tester is the cheap, always-on outside
voice, and removing it means the only clean-context reading of the work happens at the dearest tier.

**More** — a separate architect, or a separate documenter — was rejected twice over. An architect
role would author intent, which §2.4 forbids to anything running unattended. A documenter role was
considered and replaced by decision 23: the implementer writes the documentation its step binds, and
the reviewer treats a missing update as grounds for rejection. Splitting prose from code into a
fourth call would break the one property the sync table depends on — that a document changes **in
the same commit** as the code it describes.

### The role pipeline

```
guard_all + guard_tiers        every tier in the ladder must be available()
                               otherwise ProviderUnavailable → back off, no retry consumed

pick issue → resolve intent → resolve tier T (label tier:<name>, else default_tier)
git checkout -B autopilot/task-<n> autopilot/main
START_SHA = HEAD ;  claim

implement(T) ── ProviderUnavailable → release, back off, exit 0
             └─ TaskFailure         → reset, comment, attempt++, exit 0

round = 0
┌─▶ verify()                    costs no tokens. Red counts as a tester rejection
│     green → test(T, clean session) → verdict file
│                 pass   → leave the loop
│                 reject → fall through
│   round++ ;  round > max_rounds → PAUSE
│   review(T+1): adjudicate and fix → verdict file
└───────────────────────────────────┘

reviewer has not yet intervened?  → for each lens in pipeline.review_lenses:
                                       review(T+1, that lens only, fresh session)
                                       any reject → PAUSE, naming the lens
reviewer has already intervened?  → the tester's pass is the gate (decision 6)

verify()   ← final, unconditional. §2.1 has no bypass
   red   → reset to START_SHA, comment, attempt++
   green → merge --ff-only into autopilot/main → push → close issue → delete task branch
```

**When the attempt budget runs out.** `attempt++` is bounded by `pacing.max_attempts_per_issue`,
which nothing currently reads. On the attempt that exhausts it, the task does not simply fail again:
it takes the same PAUSE path as a rounds-exhausted task — the work stays on its branch, the issue is
labelled `blocked`, and the comment names every attempt and why each one ended. Only a coordinator
brings it back. A task that can be retried forever is a task that burns a usage window nightly with
nobody deciding anything.

Each role is a **fresh invocation with no session resumption**. That is what "clean context" means
for the tester, and it is free: it is the absence of `--continue` / `--resume`.

Every role reads the same two sets of documents before touching code: the project-level documents
named in `roles.context_docs` (committed — the overall plan), and the task-level `Intent:` line the
coordinator wrote onto the issue. This extends ADR-0005 rather than replacing it.

### Verdicts travel through the file system

`pi --mode json`, `opencode run --format json`, and `claude --output-format stream-json` produce
three different shapes. Parsing them for a pass/reject decision would mean a second `classify` per
harness, forever. Instead each role writes:

```
.autopilot/runs/<issue>/round-<k>/{tester,reviewer}-verdict.json
    { "verdict": "pass" | "reject", "reason": "…", "evidence": ["path:line", …] }
```

Every harness has a write tool, so this needs no adapter code at all. `.autopilot/` is already
excluded from `git reset --hard` and `git clean`, so the record survives every rejection path — which
is what makes a resumed round able to read the previous argument instead of repeating it.

**A missing or malformed verdict is a `TaskFailure` and is never read as a pass.** A reviewer that
died mid-run must not become an approval.

### The branch model

```
main                       the runner never touches it (guard unchanged)
└─ autopilot/main          accumulates verified work only
   └─ autopilot/task-<n>   one branch per task; work in progress lives here
```

| Outcome | Action |
|---|---|
| verify green | `merge --ff-only` into `autopilot/main`, push, close, delete the task branch both sides |
| **pause** | push the task branch, comment the full argument, label `blocked`, **no reset** |
| failure with attempts remaining | reset to `START_SHA`, delete the task branch; the journal remains |

This tightens something incidentally: the agent is now confined to a task branch, so
`autopilot/main` is never dirtied by an agent directly. `_settle_guard_branch` changes from *"not
main"* to *"exactly this issue's task branch"*, which is a stronger check than the one it replaces.

### The journal

Append-only JSONL at `.autopilot/runs/<issue>/journal.jsonl`, and the only interface the dashboard
has:

```jsonl
{"ts":…,"wake":"w-…","event":"wake_start","issue":42,"tier":"standard"}
{"ts":…,"wake":"w-…","event":"role_start","role":"implement","round":0,"tier":"standard","harness":"claude","model":"sonnet"}
{"ts":…,"wake":"w-…","event":"role_end","role":"implement","round":0,"tier":"standard","classify":"ok","cost_usd":0.42,"cost_source":"reported","duration_s":312}
{"ts":…,"wake":"w-…","event":"verdict","role":"test","round":0,"verdict":"reject","reason":"…"}
{"ts":…,"wake":"w-…","event":"role_end","role":"review","lens":"partial-failure","round":1,"tier":"deep","classify":"ok","cost_usd":1.90,"cost_source":"reported","duration_s":204}
{"ts":…,"wake":"w-…","event":"verify","result":"red","failed":"unit"}
{"ts":…,"wake":"w-…","event":"wake_end","outcome":"merged|failed|paused|stood_down"}
```

**A `role_start` with no matching `role_end` is a silent death**, and it is detectable with one
query — no dashboard required. This is the mechanism that answers the failure the operator hit on
2026-08-16, where sub-agents died and nothing reported it until they were asked about directly.

`ctl.sh status` — becoming `autopilot status` — reads the journal for orphaned roles, and reads
`launchctl print gui/$UID/<label>` for `runs` and `last exit code`, closing the gap
`observed-behaviour.md` identified and never filled.

**Cost honesty.** Cumulative USD per wake is only trustworthy where the harness reports it; it is
measured for `claude`, unmeasured for `pi` and `opencode`, and zero for local models. The ceiling
that is genuinely enforceable is therefore `wake_timeout_s`, which is harness-agnostic and always
measurable. `wake_budget_usd` is best-effort and is documented as such rather than presented as a
guarantee.

Every `role_end` therefore carries `cost_source`, one of `reported`, `estimated`, or `unknown`. A
figure whose provenance is not recorded beside it becomes a figure someone later trusts, and a
dashboard total that mixes reported and unknown costs without saying so is this repository's
recurring failure in a nicer font.

**Aggregation is by tier first.** The measured asymmetry — cheap roles produce most of the tokens,
the dear role produces most of the bill — means a per-wake total hides the one number an operator
can act on. The question worth answering is *which tier is the money going to, and is that the tier
that needed to do the work?*

## The skill trio, and `autopilot-planning`

Until now the product started at a queue. `docs/guides/install.md` stated the precondition and the
skill-layer design removed it for issues, but the plan those issues implement still had to exist
before anything here could help. `autopilot-planning` closes that end.

```
autopilot-planning  ──▶  autopilot-deliver  ──▶  the runner  ──▶  autopilot-review
   person present         person present          nobody watching     person present
        │                       │
        └─ plan + docs ─────────┘
           the operator commits  ▲
                                 └─ deliver's phase-1 commit gate already checks this, mechanically
```

The seam already exists. Planning stops exactly where deliver's commit gate begins, so no new
handshake is invented — planning simply has to produce what that gate already demands.

### Where this comes from

Three sources, each contributing something the others do not. Nothing below is invented for this
design.

| Source | What it contributes |
|---|---|
| **A companion project** — private, and the first user of this runner | The documentation set and, more importantly, the artifacts that keep it true: the binding sync table, the `docs/README.md` status table, and the plan template. Its rules file already describes the planner / implementer / documenter / coordinator / reviewer split this repository is automating — including *"the brief names files; the implementer reads them itself"*, which is the rule decision 2 depends on |
| **[gstack](https://gstacks.org/)** | Reviewing a plan through **distinct role lenses before approval** — `/plan-ceo-review` for the problem as the user meets it, then `/plan-eng-review` for architecture, data flow, state transitions and edge cases |
| **[superpowers](https://github.com/obra/superpowers)** | Process discipline: classify the request before answering it, ask one question at a time, propose alternatives with a recommendation, hard approval gates, and a self-review pass over the written artifact |

### The eight phases

```
0  Classify              spike | bounded | architectural                       superpowers
1  Survey                new repository → scaffold;  existing → read what is there
2  Brainstorm            one question at a time, 2–3 approaches, a recommendation
3  Design                docs/design/YYYY-MM-DD-<topic>-design.md
4  ▮ TWO-LENS REVIEW     the product lens, then the engineering lens            gstack
5  Documentation set     AGENTS.md + pointers · the docs/ tree · sync table · status table
6  Plan                  docs/plans/YYYY-MM-DD-<topic>.md, structured work breakdown
7  Self-review           placeholders · contradictions · every step carries all seven fields
8  ▮ STOP                the operator reads, approves, and commits
```

Phase 5 runs only when something is missing or has drifted; on an established project it is a check,
not a rewrite.

### The documentation set

Adopted from that project, which combines three standards rather than inventing a fourth:

```
AGENTS.md         ← the rules. The single source; other agents' rule files point here
README.md         ← what this is, how to run it
CHANGELOG.md      ← Keep a Changelog + SemVer
docs/
├── README.md         ← the map, and the STATUS TABLE for every document
├── product/          prd.md · roadmap.md (milestones + exit criteria) · glossary.md · open-items.md
├── architecture/     arc42 §1–8, §10–12
├── decisions/        ADRs, arc42 §9 — Nygard format, never edited once accepted
├── reference/        what is it — API behaviour, schema, commands
├── guides/           how do I — setup, testing, release
├── explanation/      why — conceptual background
└── plans/            what next — written before the code
```

Keep the four Diátaxis modes distinct. Blurring them is how documentation rots.

**`AGENTS.md` is canonical.** It is now a cross-tool standard governed under the Linux Foundation's
Agentic AI Foundation and read by Codex, Cursor, and Copilot among others. `CLAUDE.md`,
`.clinerules`, Kiro steering files and their equivalents are written as one-line pointers to it.
Copying the rules into each would create exactly the drift the sync table exists to prevent — five
files that disagree, and no way to tell which one an agent actually read.

### Keeping track

Four artifacts, all of them already load-bearing there, plus one addition:

| Artifact | Answers |
|---|---|
| `docs/README.md` status table | Every document, its status, and which milestone it belongs to |
| Each plan's step table | Which steps are done, which are in flight, which are the reviewer's |
| `docs/product/roadmap.md` | Milestones and their exit criteria — the gate for merging to `main` |
| `docs/product/open-items.md` | **New.** Questions with no answer yet, and accepted technical debt |
| `AGENTS.md` sync table | Change *this*, and you must update *that*, in the same commit |

A single root `STATUS.md` was rejected: every commit would touch it, and with one branch per task
that is a merge conflict per task.

### The structured work breakdown

This is what makes a plan a contract between the two skills rather than prose one of them
interprets. Every step declares seven fields, and every field has exactly one consumer:

```markdown
### Step 7 — Add the opencode adapter
- **Done when:**   `available()` and `models()` return correctly on a machine with opencode installed
- **Verify:**      `cargo test harness::opencode` → six assertions green
- **Intent:**      docs/design/2026-08-16-multi-harness-role-pipeline-design.md
- **Depends on:**  Step 4
- **Tier:**        standard
- **Needs human:** no
```

| Field | Consumed by |
|---|---|
| Done when · Verify | the issue body, and the `verify` commands the runner executes |
| **Intent** | `queue_intent`. A task without it is refused **before a single token is spent** (ADR-0005) |
| Depends on | the creation order in deliver's phase 5 |
| Tier | the `tier:<name>` label |
| Needs human | the `needs-human` label, which leaves the issue open for a person |

Phase 7 can therefore check mechanically: a step missing `Verify` or `Intent` is an issue the runner
will refuse at 3 a.m., and catching it while writing the plan costs nothing.

### Who writes the documentation, with nobody watching

The companion project forbids its implementer from writing any Markdown, because a human-present
Claude Code session writes it instead. That option does not exist here.

**The implementer writes the documentation its step binds, and the reviewer treats a missing update
as grounds for rejection.** The plan's *Documentation to update* list names the obligation, so it is
neither discovered nor invented at run time — it was approved with the plan. This is what makes the
sync table enforceable unattended, and it is why the reviewer sits at a higher tier: judging whether
a document still tells the truth is harder than writing the code it describes.

The runner's prompt keeps its existing prohibitions — it does not write plans, does not modify
`AGENTS.md`, and does not alter an accepted ADR. Those are intent, and §2.4 reserves intent for a
person.

## Preflight in `autopilot-deliver`

Phase 2 (*Prepare the project*) gains tier resolution, and follows the rule already established there
for `verify`: **detect, propose, the operator confirms, then write** — never decide silently.

```
for each harness adapter:  available()  → binary present? authenticated?
                           models()     → which models are actually reachable
                        ↓
propose a ladder → operator edits and confirms
                        ↓
write  config.json                (tier names · roles · pipeline)     committed
write  .autopilot/tiers.local.json (tier → harness/model/effort/$)    gitignored
                        ↓
re-check: every tier resolves → otherwise STOP, naming exactly what is missing
```

The ladder is re-detected on **every** deliver run, not written once and trusted. The runtime
counterpart is `guard_tiers`, which runs inside `guard_all` on every wake: cheap, and it turns "LM
Studio was switched off at 3 a.m." into `ProviderUnavailable` — a back-off that consumes no retry
budget and fails no task.

Phase 3 (*Shard*) changes only in what it writes onto each issue: `tier:<name>` in place of `model:`
and `effort:`.

## The dashboard

Go, htmx, read-only, outside `runner/`, never invoked by the runner. It scans the installed projects
already enumerated by `~/.local/share/autopilot/jobs/*.plist`, reads their journals, and serves:

- wakes in flight, with the role currently running and how long it has been running
- **orphaned roles** — a `role_start` with no `role_end`; the headline feature
- **cost and wall-clock aggregated by tier first**, then by role, then by wake — with `reported`,
  `estimated`, and `unknown` costs shown apart and never summed into one figure
- tasks paused and waiting for a coordinator, with the argument that paused them
- the circuit breaker, `STOP`, and `launchctl` runs / last exit code per project

htmx is vendored as a single file and embedded with `go:embed`, so there is no build step beyond
`go build` and no package manager in the loop. The source ships and is read; the binary is an
artifact of it.

## The invariants

The four in `CLAUDE.md` §2 survive. Three are strengthened, one is generalised, and four more are
added because this design creates the ways to break them.

| # | Invariant | Change |
|---|---|---|
| 1 | Verification gates the commit | **Strengthened.** It now also runs at every loop boundary, and the final run before the merge is unconditional |
| 2 | Usage exhaustion is not a task failure | **Generalised** to `ProviderUnavailable`: closed usage window, dead endpoint, or expired token. None consumes a retry |
| 3 | State round-trips | **Widened** to the journal and to paused task branches. A paused task must be resumable from what is on disk and on the remote |
| 4 | The runner executes intent, it never authors it | **Unchanged and reinforced.** The coordinator is human-present; escalation is a pause, not a fourth unattended agent |
| 5 | A missing or malformed verdict is never a pass | New — the direct consequence of decision 12 |
| 6 | The agent never works on `autopilot/main`, only on a task branch | New — the direct consequence of decision 7 |
| 7 | No harness name appears outside `runner/src/harness/` | New — §3 portability, extended from projects to harnesses |
| 8 | The runner never references a skill or the dashboard | New — the widened boundary |

## Testing

The discipline in `CLAUDE.md` §4 carries over intact, with the language changed:

- Tests run against a **real throwaway git repository**. Git's behaviour is still the thing relied on.
- `gh` and every agent CLI are stubbed with fixture programs on `PATH`. Output shapes are **recorded
  from real runs**, never invented — which is why an unproven adapter ships labelled unproven.
- Every invariant above carries a test that fails if the invariant is removed. Invariants 7 and 8 are
  grep tests, as their shell predecessor already is.
- `clippy` clean, `cargo fmt` clean, and `go vet` clean for the dashboard, before commit.

Two classes need tests that did not exist before:

- **Pipeline exit paths.** Every way a task can end must be exercised — guard stand-down, unavailable
  provider, intent refusal, failed claim, implement failure, rounds exhausted, attempts exhausted,
  final-review rejection, red verify, and success — including the pause-and-resume round trip and the
  reviewer-already-intervened branch of decision 6.
- **The shell runner as reference.** During the cutover the existing suite becomes a conformance
  suite: the same fixture repositories, the same stubs, run against both binaries, compared. That is
  what decision 15 buys, and it ends when the shell is deleted.

Skills remain Markdown and remain untestable as units; they are verified end to end against a
throwaway repository, as today.

## Effect on existing documents

- `CLAUDE.md` §1 — Rule Zero's *POSIX shell only* is replaced. The reason it gives is kept verbatim
- `CLAUDE.md` §2 — four invariants become eight; two are amended
- `CLAUDE.md` §3 — the five project-specific concerns become six, adding the tier ladder, and the
  "no project name in `lib/`" rule extends to "no harness name outside `harness/`"
- `CLAUDE.md` §4 — Rust and Go tooling replaces `shellcheck`
- `docs/decisions/` — a new ADR supersedes Rule Zero's language constraint. ADR-0001, 0002, 0004, and
  0005 are unaffected; ADR-0003 was already superseded
- `docs/design/2026-08-15-skill-layer-design.md` — its boundary sentence survives; its architecture
  diagram gains `dashboard/` and the runner becomes a compiled binary
- `docs/reference/observed-behaviour.md` — gains the three harness measurements above, dated
- `docs/guides/install.md` — gains tier configuration and the preflight; loses `jq` as a dependency
- **`CLAUDE.md` becomes a pointer to a new `AGENTS.md`** in this repository too, per decision 20.
  This repository must follow the convention its own planning skill installs elsewhere
- `skills/autopilot-deliver/SKILL.md` — phase 2 gains tier preflight; phase 3 reads the structured
  work breakdown instead of interpreting prose, and writes `tier:` in place of `model:` / `effort:`
- `skills/autopilot-review/SKILL.md` — gains the journal as a seventh source, above all the orphaned
  roles it makes visible

## Out of scope

- **Running roles in parallel.** The pipeline is sequential by construction: the tester reads what
  the implementer wrote, and the reviewer adjudicates between them.
- **More than one task per wake.** ADR-0001 stands; a task is simply larger now.
- **A runtime coordinator.** Decision 2 places it with a person. An unattended agent that decides
  whether to close or requeue a disputed task is exactly what §2.4 forbids.
- **The dashboard writing anything.** It reads journals. Acting on what it shows is done through the
  existing skills or by hand.
- **Re-sharding after a plan changes.** Still a real problem, still a different one.
- **Planning committing anything.** Decision 18. It writes and stops; the commit is the operator's
  approval and the gate deliver checks.
- **Migrating an existing project's documentation wholesale.** Phase 5 scaffolds what is missing and
  reports what has drifted. Rewriting a large existing documentation set is a task for a plan, not a
  side effect of running a skill.
- **Adapters for harnesses that cannot be run on the developing machine.** The contract accommodates
  them; the plan does not claim them.

## Still unverified

Carried forward, and added to:

- A reboot, and intent binding end to end against the real agent — both open since 2026-08-15
- `pi --mode json` and `opencode run --format json` output shapes — never measured; measuring them is
  a prerequisite step in the plan, and both require fixing this machine first (`opencode`'s invalid
  config, `pi`'s missing credentials)
- Whether `opencode` reaches `ollama` and `lmstudio` in practice — documented, unmeasured here,
  and the only route to a local-model tier now that `pi` is confirmed not to support them
- The throttled `rate_limit_event` payload — still only `{"status":"allowed"}` has ever been seen
