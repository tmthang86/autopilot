---
name: autopilot-planning
description: Turn an idea into a committed plan the unattended runner can be given — brainstorm, design, then plan. Use at the start of a project or a milestone, before autopilot-deliver. Also use when there is time but no particular idea: it proposes what is next from the project's own registers rather than asking a blank question.
---

# autopilot-planning

The front of the chain. It takes an idea, argues it into a design, installs the documentation
convention if the project lacks it, and writes a plan whose steps can be sharded into issues without
interpretation. It is the first of three: **`autopilot-planning` → `autopilot-deliver` →
`autopilot-review`.**

## When to use

- A project or a milestone is starting and there is no plan yet.
- A project has a plan but not the documentation set the delivery chain expects.
- **The operator has time and no particular idea.** Phase 0 proposes what is next from the project's
  own registers rather than asking a blank question.

## It writes. It does not commit.

**The operator commits.** `autopilot-deliver` phase 1 checks mechanically that the plan is tracked
and clean, and that check is the only approval a tired operator cannot wave through. A skill that
commits its own plan deletes it. Say so plainly when stopping: which files were written, and that
committing them is the approval.

**A note on rule-file convention.** In target projects this skill installs `AGENTS.md` as canonical
and `CLAUDE.md` as a pointer. This repository itself keeps `CLAUDE.md` canonical with `AGENTS.md` as
the pointer, because a `claude` session loads `CLAUDE.md` automatically (decision 39). For a
claude-first target project, consider the same arrangement.

## Flow

```
0  Identify the work      idea given → use it;  none given → propose from the project itself
   then classify         spike | bounded | architectural
1  Survey                new repository → scaffold;  existing → read what is there
2  Brainstorm            one question at a time, 2–3 approaches with a recommendation
3  Design                3a research prior art -> 3b name uncertainties ->
                        3c prove them with a committed PoC -> 3d only then draft
4  ▮ TWO-LENS REVIEW     fresh sub-agent PER LENS — product, then engineering
5  Documentation set     AGENTS.md + pointers · the docs/ tree · sync table · status table
6  Plan                  docs/plans/YYYY-MM-DD-<topic>.md, structured work breakdown
7  Check                 7a mechanical here -> 7b fresh sub-agent -> 7c triage every
                        finding -> 7d at most ONE re-review, then stop
8  ▮ STOP                what was written · what review found · what is unproven ·
                        the operator reads, approves, and commits
```

## Phase 0 — identify the work, then classify it

**If the operator arrived with an idea, use it.** If they arrived with nothing, do not ask *"what
would you like to build?"* — the project already knows. Read the tracking artifacts and **propose
candidates**, newest evidence first:

| Source | What it offers |
|---|---|
| `docs/product/roadmap.md` | The next milestone whose predecessor has closed, and its exit criteria |
| `docs/product/open-items.md` | Unverified guarantees, open questions, and accepted debt |
| open issues labelled `blocked` | Questions the runner stopped on and nobody has answered |
| open `needs-human` issues with commits | Work finished but never accepted |
| `docs/README.md` status table | Documents marked Pending whose milestone has arrived |

Present them as a short list with what each would cost and why it might be next, and let the
operator choose. Only when every one of those is genuinely empty — a new repository with no history
— ask what they want to build.

**Then classify, and say the classification out loud.**

- **Spike** — a feasibility question whose output is an answer, not code. No design, no plan, and
  **nothing reaches the delivery chain**. Report the finding and stop.
- **Bounded** — a well-scoped change to a flow that already exists. **It still produces a plan
  file**, because the plan file is the interface to `autopilot-deliver`. What shrinks is everything
  around it: fewer questions in phase 2, one lens instead of two in phase 4, and the design becomes
  a paragraph in the plan's *Context*. One to three tasks is normal.
- **Architectural** — a new project, a new subsystem, or a change to how components fit together.
  The full flow below.

**Bounded and architectural differ in size, never in kind.** A bounded change that skipped the plan
would have to be implemented by hand. If a change is genuinely too small to be worth an issue, say
so plainly and let the operator do it directly.

## Phase 1 — survey

Read before proposing. On an existing project: the current `AGENTS.md` or `CLAUDE.md`, `docs/`,
recent commits, and whatever `docs/reference/` records about behaviour learned the hard way. On a
new one: there is nothing to read, and phase 5 will scaffold. State which case it is.

## Phase 2 — brainstorm

One question per message. Prefer multiple choice. Understand purpose, constraints, and what would
count as success before proposing anything. Then propose two or three approaches with trade-offs,
lead with a recommendation, and say why. YAGNI ruthlessly.

## Phase 3 — the design, and nothing uncertain enters it unproven

The most expensive mistake available here is a design that reads well and rests on a technical
claim nobody checked. So phase 3 has four steps, and drafting is the last of them.

### 3a — find out how this has already been solved

**Do this first, before naming a single uncertainty of your own.** Search the web for the class of
problem, not just for the API you happen to be reaching for. Bring back three things, and put all
three in the design: **what to adopt** (named and cited), **what to refuse and why**, and **what
corroborates or contradicts** the design's own reasoning. An idea rejected without a recorded reason
gets adopted later by someone who does not know it was considered.

### 3b — name the uncertainties out loud

List every technical claim the design will rest on that you do not *know* to be true. An uncertainty
nobody named is one nobody proved. Search again for each, this time narrowly. **Documentation is a
lead, never a verdict** — when a claim can be measured, measure it.

### 3c — prove what is left with a PoC

For every uncertainty that survives 3b, write **the smallest script that fails if the assumption is
wrong**, run it, and read the output. Commit it:

```
docs/design/poc/YYYY-MM-DD-<topic>/
├── <name>.sh          the probe, re-runnable by anyone
└── RESULT.md          the command, the real output, the date, and what it settled
```

Record failures too, and record them first. Where the finding is about an external tool's behaviour,
it also goes into `docs/reference/observed-behaviour.md`.

### 3d — then draft

Write to `docs/design/YYYY-MM-DD-<topic>-design.md` from `templates/design.md.tmpl`. Every claim in
the finished design is one of three things, and **which one must be visible**: *measured* (dated),
*cited* (with URL), or *neither* (listed in *Still unverified*). **A design may contain an unproven
claim. It may never contain an unmarked one.**

## Phase 4 — the two-lens review, by an outside voice

Before the design is approved it is read by **fresh sub-agents, one per lens** — not re-read by the
session that wrote it. The product lens asks *is this the problem worth solving, as the user meets
it?* The engineering lens asks *where does this fail under concurrency, partial failure, or a
restart; what is asserted rather than checked?*

**How the brief is written, and this part is easy to get wrong:**

1. **Name paths; never paste excerpts.** The agent reads the design and the documents it cites
   itself.
2. **Never hand over the conversation.** No transcript, no summary of what was decided.
3. **State the question, never the expected answer.** "Check the design handles a restart" is a
   question; "confirm it does" is an instruction to agree.

Each lens reports what it found, **including finding nothing**. Findings go **to the operator**, not
into the design automatically.

## Phase 5 — the documentation set

Only what is missing or has drifted. On an established project this is a check, not a rewrite.

- `AGENTS.md` from `templates/AGENTS.md.tmpl` — **the single source of rules** in the target project;
  other rule files become one-line pointers to it.
- The `docs/` tree, `docs/README.md` with its status table, and `docs/product/open-items.md`.
- The **sync table** in `AGENTS.md` §4: this project's own bindings, not a generic list. Propose
  rows from what the repository actually contains; the operator confirms.

Rewriting a large existing documentation set is a task for a plan, not a side effect of running a
skill. Report the drift and stop.

## Phase 6 — the plan

From `templates/plan.md.tmpl`, to `docs/plans/YYYY-MM-DD-<topic>.md`. Every task carries a goal in
its heading and six fields — `Done when`, `Verify`, `Intent`, `Depends on`, `Tier`, `Needs human` —
each with exactly one consumer in the delivery chain. Steps inside a task are bite-sized and contain
the actual test code, the actual implementation, and the actual commit message. **Mark a task
`Needs human: yes`** when its correctness depends on behaviour a person has to observe.

## Phase 7 — mechanical check, then a second outside voice

### 7a — the mechanical pass, run by this session

```sh
sh <skill-dir>/validate-plan.sh docs/plans/<the-plan>.md <project-root>
```

Exit 0 and no output, or fix what it names. Then read for what a script cannot see: placeholders
(`TBD`, `TODO`, "add error handling"), names that drift between tasks, and steps that say *what*
without showing *how*.

### 7b — the outside voice, on the design and the plan together

A fresh sub-agent, given the paths to **the design, the plan, and the project's rules**, reading all
three itself. The same three brief rules as phase 4 apply unchanged. It answers questions the writer
structurally cannot: does the plan implement the design; could an engineer with no context execute
each task as written; is anything presented as settled that was never proven; what is missing
entirely.

**When 7b may be skipped.** On the **bounded** path, when the plan is one to three tasks and 7a
found nothing. Say so out loud rather than skipping quietly. On the architectural path it is never
skipped.

### 7c — what happens to a finding

Every finding from phase 4 or 7b gets exactly one of three dispositions:

| Disposition | What it requires |
|---|---|
| **Accepted** | The change, made. If it touches an assumption a PoC settled, that PoC is re-run |
| **Rejected** | **Evidence, not argument** — a file, a measured output, or the line the reviewer missed |
| **Undecided** | Handed to the operator, and the design's *Still unverified* gains a row |

A rejection needs evidence; an acceptance needs a re-run. Neither is the lazy default.

### 7d — at most one re-review, then stop

If accepted findings changed the design or the plan materially, run the affected review **once more**
with a fresh agent, scoped to what changed. **One extra round, never a loop.** Write the ledger to
`docs/design/YYYY-MM-DD-<topic>-review.md`:

```markdown
| # | Lens | Finding | Disposition | Evidence or change |
|---|---|---|---|---|
```

## Phase 8 — stop

State four things, in this order, and then stop.

1. **What was written.** Every path, including the PoCs and the review ledger.
2. **What an outside voice found, and what was done about it.** The counts and the disagreements:
   *"Two lenses and one plan review: 9 findings. 6 accepted, 2 rejected with evidence (ledger rows 3
   and 7), 1 undecided and now in Still unverified."*
3. **What is still unproven.** Read the design's *Still unverified* section out loud.
4. **That nothing was committed, and that committing is the approval:**

> Nothing is committed. Committing these files is the approval, and `autopilot-deliver` checks for
> it mechanically before it will shard anything.

Then stop. Do not offer to commit. Do not invoke `autopilot-deliver`. **If a finding was rejected,
say which and on what evidence.**
