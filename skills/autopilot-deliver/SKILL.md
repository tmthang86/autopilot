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
     fail → stop and name what is missing. No override.
2  Prepare the project (each part skipped if already done)
     deploy the runner (deploy.sh aborts if the target is a git checkout)
     autopilot not installed here?  → install-project.sh
     labels missing?                → gh label create
     verify still the template?     → detect project type, propose, operator confirms
     config.json uncommitted?       → block; git reset --hard would discard it
3  Shard — propose the issues
     each: goal · done-when · verify block · Intent: · Depends on · model/effort · needs-human
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

## Prepare steps

Before sharding, confirm the project is ready:

- The runner is deployed to `~/.local/share/autopilot/` and `ctl.sh status` prints a `runner:` line.
- `install-project.sh` has created `.autopilot/config.json` and the nine labels.
- The `verify` block names commands this project actually uses, not the `example` placeholder.
- `config.json` is committed — an uncommitted edit is reverted by the first rejection path.

If any is missing, do the smallest thing that closes that gap and keep the operator informed. Do not
silently assume a language, build tool, or verify command: propose, confirm, then write.