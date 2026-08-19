---
name: autopilot-deliver
description: Turn an approved plan into reviewed GitHub issues the unattended runner will execute. Use when a committed plan is ready to be delivered as labelled, dependency-ordered work for autopilot.
---

# autopilot-deliver

Takes an approved plan and turns it into the issues the unattended loop will run, one milestone at
a time. It installs autopilot into the project when needed. It never starts the loop — the operator
does that, explicitly.

## When to use

- A plan exists under `docs/plans/` and is committed, and the operator wants it sharded into issues.
- A project needs autopilot installed and prepared before sharding.

Do not write a plan from scratch and immediately shard it in the same session. Phase 0 terminates
when no plan exists; the operator commits the plan, then invokes this skill again.

## Flow

```
0  Identify the plan
     path given     → use it
     none given     → list docs/plans/*.md and ask
     none exist     → write one, then STOP
1  ▮ COMMIT GATE — checked mechanically, never asked
     git ls-files --error-unmatch <plan>   must be tracked
     git status --porcelain <plan>         must be empty
     validate-plan.sh <plan> <root>        must exit 0
     fail → stop and name what is missing. No override.
2  Prepare the project (each part skipped if already done)
     deploy the runner (deploy.sh aborts if the target is a git checkout)
     autopilot not installed here?  → autopilot install
     labels missing?                → gh label create
     verify still the template?     → detect project type, propose, operator confirms
     TIER LADDER not established?   → autopilot preflight → propose → confirm → write
     any tier unresolved?           → STOP, naming the harness or model that is missing
     config.json uncommitted?       → block; git reset --hard would discard it
3  Shard — transcribe, do not interpret
     each task in the plan becomes one issue, field by field:
       heading    → issue title
       Done when  → the done-when section of the body
       Verify     → the verify block
       Intent     → the Intent: line
       Depends on → Depends on #<num>, resolved in phase 5
       Tier       → label tier:<name>
       Needs human→ label needs-human
     also write .autopilot/proposed-issues.json (gitignored) for crash recovery
4  Review — in conversation
     revise until the operator approves: "merge 3 and 4, drop 7, make 5 needs-human"
5  Create
     gh issue create in dependency order, rewriting indices into real issue numbers
6  ▮ START GATE — asked
     state what will run, at what interval, and how to stop it
```

## The two hard gates

**Commit gate (phase 1).** The plan must be committed before sharding. This is checked with git,
not by asking whether it happened. A plan that is not tracked, or has uncommitted changes, stops the
flow naming exactly what is missing. There is no override.

**Start gate (phase 6).** Starting the loop is the operator's decision and is always asked, never
done. State the project, the interval, and the exact stop command (`ctl.sh stop`).

## Issue format

Every issue the skill creates carries an `Intent:` line naming at least one committed file, because
the runner refuses a task without one (ADR-0005). Each affected document goes on the same line,
space-separated:

```
Intent: docs/plans/2026-08-15-initial-design.md docs/reference/upstream-api.md
```

Dependencies are declared with `Depends on #<num>`, where the number is the real issue number
learned only during phase 5 — the proposal uses indices, and creation rewrites them.

The tier comes from the task's **Tier** field and becomes a `tier:<name>` label, where the name must
be one the project's ladder declares in `config.json`. The model and effort labels are gone;
the ladder in `.autopilot/tiers.local.json` binds a tier name to a harness and a model. A `Tier`
naming something outside `config.tiers` is a **plan error, not a label to create** — creating
`tier:medium` because a plan said so produces an issue the runner picks up and then cannot resolve.

## Establishing the tier ladder

Run `autopilot preflight` and read its JSON. **Do not run the CLIs yourself** — the adapters are the
one detector, and a second one written here would disagree with the one the runner uses at 3 a.m.

Propose a ladder from what came back, ordered cheapest first, and say what each entry costs:

    light     opencode  <a free or local model>        $0
    standard  claude    sonnet                         ~$5
    deep      claude    opus                           ~$15

Three rules when proposing:

- **Never propose a harness whose `proven` is false without saying so.** An adapter that has never
  run against its real CLI is a guess with a type signature.
- **Never invent a model name.** Propose only from the `models` list preflight returned.
- **The reviewer's tier is one step up the ladder**, so a two-entry ladder means the top tier
  reviews itself. Say that out loud; it is a real weakening and the operator should choose it
  knowingly rather than discover it.

The operator edits and confirms. Then write:

- `.autopilot/config.json` → `tiers`, `roles`, `pipeline`   — **committed**
- `.autopilot/tiers.local.json` → the bindings              — **gitignored**

Re-run `autopilot preflight` afterwards. If `unresolved` is non-empty, **stop** and name each entry
with the harness or model that is missing and what would fix it. There is no override: a ladder that
does not resolve is a loop that stands down every night, and that looks exactly like a quiet queue.

## Prepare steps

Before sharding, confirm the project is ready:

- The runner is deployed to `~/.local/share/autopilot/` and `autopilot status` prints a `runner:` line.
- `autopilot install` has created `.autopilot/config.json`, the labels, and the gitignore entries
  (including `.autopilot/tiers.local.json`).
- The `verify` block names commands this project actually uses, not the `example` placeholder.
- The tier ladder resolves: `autopilot preflight` reports `unresolved` empty.
- `config.json` is committed — an uncommitted edit is reverted by the first rejection path.

If any is missing, do the smallest thing that closes that gap and keep the operator informed. Do not
silently assume a language, build tool, verify command, harness, or model — propose, confirm, then write.