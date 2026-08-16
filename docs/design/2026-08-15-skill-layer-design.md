# The skill layer — design

> **Type:** Design · **Date:** 2026-08-15 · **Status:** Approved
> **Scope:** How an approved plan reaches the unattended loop, and how the loop's output reaches the
> operator. Covers two new skills, the plugin distribution model, and the shell changes they need.

## Context

The runner works and has never been used on a second project. `docs/guides/install.md:8-9` states
the reason as a precondition: *"its work must already exist as issues carrying the `autopilot`
label."* Producing those issues is the largest part of adopting the runner and the one part it does
not help with. Creating the labels it queries is another, and its absence fails silently —
`lib/queue.sh:28-30` cannot distinguish a missing label from an empty queue.

The other end is missing too. The loop runs overnight and leaves logs, issue comments, commits on
`autopilot/main`, and `needs-human` issues waiting for a person. Nothing helps read any of it, and
nothing merges `autopilot/main` back to `main` or reminds anyone to.

So the runner is a well-tested engine with no intake and no exhaust. This design builds both, as
skills rather than shell.

## Decisions taken

Recorded here because they shape everything below. Each was an operator decision made on
2026-08-15, not an inference.

| # | Decision | Consequence |
|---|---|---|
| 1 | The skill installs and prepares, then **asks** before starting the loop | ADR-0002 survives intact — a human still explicitly starts it. No superseding ADR needed |
| 2 | The skill does the sharding; the autopilot repository ships the skill | No `shard.sh`. Review happens in conversation, not by editing JSON |
| 3 | If no plan exists the skill may write one, but **stops** — the plan must be committed before sharding, checked mechanically | Two approvals stay separate acts. See *The commit gate* |
| 4 | Two skills, split by rhythm: `autopilot-deliver` per milestone, `autopilot-review` per morning | Setup folds into deliver; it runs once per project and needs no door of its own |
| 5 | Distribution is a Claude Code plugin; the runner is copied out to a stable path | `/plugin update` updates both halves. The launchd path never moves |

## Architecture

```
~/.local/share/autopilot/          the deployed runner — stable path, referenced by launchd
├── run-once.sh, lib/, ctl.sh      the unattended loop. POSIX shell. Unchanged by this design
├── install-project.sh             + label creation, + shared job-label derivation
└── VERSION                        compared against the plugin's, to detect a stale copy

<plugin>/                          delivered by /plugin install, version-stamped cache path
├── .claude-plugin/plugin.json
├── runner/                        the source of the above; copied out on first use
└── skills/
    ├── autopilot-deliver/SKILL.md
    └── autopilot-review/SKILL.md
```

### The boundary

One sentence governs it: **skills run when a person is present; shell runs when nobody is.**

The bridge is never built in the other direction. `run-once.sh` and `lib/` must not invoke a skill,
because a scheduled run has no operator, and a skill that a scheduled run could call would let the
loop reach the one capability reserved for supervised moments.

This is checkable rather than promised:

```sh
grep -rl "skills/" run-once.sh lib/     # must be empty, permanently
```

### Why the runner is copied out rather than run in place

Plugin cache paths carry the version: `.../<plugin>/<version>/`. `install-project.sh:54` writes an
absolute path to `run-once.sh` into each project's launchd plist. A runner living in the cache would
mean every `/plugin update` silently invalidated every installed job — and per ADR-0004, launchd
failures of this kind announce themselves only as work that quietly stops happening.

The copy costs a staleness check, which is a `VERSION` comparison the skill makes on every run and
`ctl.sh status` reports.

### The deployed copy is not the source repository

This repository becomes the plugin. `~/.local/share/autopilot/` becomes a **deployment**, holding
only what a scheduled run needs:

```
run-once.sh · lib/ · ctl.sh · install-project.sh · templates/ · VERSION
```

`docs/`, `tests/`, `CLAUDE.md`, and git history stay in the plugin repository, where development
happens. The deployed copy carries no `.git` and is never edited in place — a fix is made in the
repository, released, and delivered by `/plugin update`. This also serves `CLAUDE.md` §1: the thing
running unsupervised on the machine is now smaller than the thing you read.

**Migration of the existing install.** On this machine that path is currently a git repository, and
overwriting it would destroy work. The skill must therefore refuse to write over a directory
containing `.git` and say what it found. Converting an existing install is a deliberate one-time act
by the operator — push the repository to its remote, delete the directory, let the skill deploy —
and never something a skill does on its own initiative.

## `autopilot-deliver`

Six phases, two hard gates.

```
0  Identify the plan
     path given      → use it
     none given      → list docs/plans/*.md and ask
     none exist      → write one, then STOP

1  ▮ COMMIT GATE — checked, not asked
     git ls-files --error-unmatch <plan>    must be tracked
     git status --porcelain <plan>          must be empty
     fail → stop and name what is missing. No override

2  Prepare the project (each part skipped if already done)
     autopilot not installed here?  → install-project.sh
     labels missing?                → gh label create
     verify still the template?     → detect project type, propose, operator confirms
     config.json uncommitted?       → block; git reset --hard would discard it

3  Shard — propose the issues
     each: goal · done-when · verify block · Intent: · Depends on · model/effort · needs-human

4  Review — in conversation
     "merge 3 and 4, drop 7, make 5 needs-human" → revise, present again

5  Create
     gh issue create in dependency order, rewriting indices into real numbers

6  ▮ START GATE — asked
     state what will run, at what interval, and how to stop it
```

### The commit gate

Decision 3 allows the skill to write a plan when none exists. That is convenient and it is also the
weak point of this design: two approvals — *the plan is right* and *these issues implement it* —
land in one conversation, minutes apart, where the second tends to inherit the first's momentum.

Phase 0 therefore terminates. The skill writes the plan and stops; the operator reads it, commits
it, and invokes the skill again. Phase 1 then verifies the commit **mechanically** rather than
asking whether it happened. A machine check is stronger than a nod, and it is the only part of this
that a tired operator cannot wave through.

### Why `verify` detection matters more than it looks

`verify` is the whole of what "works on any project" means in practice, and `docs/guides/install.md`
already names it as the only thing standing between the agent and a bad commit. Phase 2 detects the
project type — `Cargo.toml`, `package.json` and its **actual** `scripts` block rather than an
assumed one, `pyproject.toml`, `go.mod` — and proposes. It never decides: the operator confirms.

### Crash safety

Phase 3 also writes `.autopilot/proposed-issues.json`, gitignored. Not as the review surface — the
review is the conversation — but so a session that dies mid-review has not thrown the work away.

## `autopilot-review`

Reads six sources and produces a brief, then walks the operator through the decisions only a person
can make.

| Source | Answers |
|---|---|
| `.autopilot/logs/<date>.log` | what the loop decided, tick by tick |
| `git log autopilot/main --since` | what landed, and why — the commit bodies carry the reasoning |
| open issues labelled `blocked` | the questions the runner stopped to ask |
| open `needs-human` issues with commits | work waiting on a person's eyes |
| `state.json` | `tasks_today`, `consecutive_failures`, `resume_after` |
| `.autopilot/STOP` | whether the circuit breaker tripped |

Four situations it handles:

**The circuit breaker tripped.** It does not offer to remove `STOP` first. It shows the last three
failure comments, because `docs/guides/install.md:107` records that three consecutive failures are
usually one broken assumption rather than three unlucky tasks.

**A `blocked` issue.** It shows the question the runner stopped on. The operator answers, the skill
comments and removes the label. This is the only route back into the queue for a blocked task.

**A `needs-human` issue with commits.** This is the Definition-of-Done gap — *exercised in the real
app* — and the skill **must not close these on its own judgement.** It states precisely what to open
and look at, and what the issue claims was done. The operator decides.

**Nothing happened at all.** It distinguishes the three causes in `docs/guides/install.md:96-101` —
never started, `STOP` present, or `gh`/`jq` absent from the plist's `PATH` — instead of leaving the
operator to guess.

**And the missing end of the loop:** nothing merges `autopilot/main` into `main`, and nothing
reminds anyone to. The runner is forbidden from touching `main`, so this is a human act with no
prompt attached to it. The morning brief is where that prompt belongs — *"12 commits on
`autopilot/main`, verification green on all of them, merge?"*

## Shell changes required

Smaller than the superseded `project-bootstrap` plan, because the skill absorbs the sharding.

| Change | Reason |
|---|---|
| Label creation in `install-project.sh`, idempotent | A fresh repository has none, and their absence fails silently |
| `lib/label.sh` — derive the job label from the `origin` remote | `install-project.sh:26` uses `basename`, so `~/work/api` and `~/side/api` collide and the second install overwrites the first's plist |
| `queue_candidates()` distinguishes a missing label from an empty queue | `lib/queue.sh:28-30` currently reports both as `[]` |
| Intent binding | Specified separately in `docs/plans/2026-08-15-intent-binding.md`; the skill produces the `Intent:` line the runner consumes |
| `ctl.sh status` reports a stale runner copy | The cost of copying out of the version-stamped plugin path |

## Errors and testing

Shell keeps the discipline already in `CLAUDE.md` §4: real throwaway git repositories, `gh` and the
agent CLI stubbed from recorded fixtures, `shellcheck` clean, and every invariant carrying a test
that fails if the invariant is removed. The boundary check above becomes one of those tests.

Skills are Markdown and cannot be unit tested. Stating that plainly is better than pretending
otherwise: they are verified by running them end to end against a throwaway repository, which is
what the shell suite already does.

Three failure modes are designed for rather than discovered:

- **Plugin updated, deployed runner stale.** `VERSION` comparison on every skill run; the skill
  re-copies, `ctl.sh status` reports it.
- **A skill runs while a scheduled run is in flight.** Creating issues is harmless; rewriting
  `config.json` is not. The skill checks the lock `guard_lock` holds before touching config.
- **`gh` not authenticated.** Detected at the start with one clear sentence, rather than surfacing
  as an empty queue or a failed create halfway through phase 5.

## Effect on existing documents

- `docs/plans/2026-08-15-project-bootstrap.md` — **superseded by this design.** Its `shard.sh` is
  dropped; its label creation, label-collision fix, and silent-failure fix survive in the table above
- `docs/plans/2026-08-15-intent-binding.md` — unaffected and still needed. This design is what
  produces the `Intent:` lines it consumes
- `CLAUDE.md` §3 — four project-specific concerns become five, and portability now includes queue
  preparation
- `CLAUDE.md` §5 — records `docs/design/`, which this document creates
- `docs/guides/install.md` — the precondition that issues must already exist is removed

## Out of scope

- **Writing or approving plans as part of delivery.** Phase 0 may draft one, but it stops there;
  approval is a separate act by a person, verified as a commit.
- **Sharding from inside the scheduled loop.** The operator's presence is what makes it safe.
- **Re-sharding after a plan changes.** It requires reconciling against issues that already exist
  and may be closed — a real problem, and a different one.
- **Replacing the loop's own supervision.** `autopilot-review` reports and asks; it never closes a
  `needs-human` issue, removes `STOP`, or merges to `main` without being told to.
