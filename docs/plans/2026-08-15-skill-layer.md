# Skill layer implementation — deliver, review, and deployment

> **Type:** Plan · **Date:** 2026-08-15 · **Status:** Done
> **Scope:** Shell deployment step, the two operator skills, plugin packaging. No change to the loop's guards, verify, or pacing.

## Context

The runner is a tested engine with no intake and no exhaust (`docs/design/2026-08-15-skill-layer-design.md`).
This plan builds both ends: `autopilot-deliver` turns an approved plan into labelled issues, and
`autopilot-review` turns the overnight loop's output into a short morning brief. It also builds the
deployment step that copies `runner/` out to the stable path installed launchd jobs point at.

The design doc and ADR-0005 already settled the shape of every decision this plan touches. This
plan exists to break that shape into tasks with concrete, testable outputs.

## What we already know

| Fact | Source |
|---|---|
| The unattended loop must never reach into `skills/`, and that is checkable by grep | design §The boundary; already tested at `tests/test_run_once.sh` |
| The runner is copied out of the version-stamped plugin cache to a stable path | design §Why the runner is copied out |
| The deployed copy carries no `.git` and is never edited in place | design §The deployed copy is not the source repository |
| A skill must refuse to write over a directory containing `.git` | design §Migration of the existing install |
| `ctl.sh status` already prints `runner/VERSION` | plugin-foundations plan Task 2, committed |
| Intent is bound by the plan that preceded this one | ADR-0005, committed `e250a59` |
| Skills are Markdown and cannot be unit tested; they are verified end-to-end | design §Errors and testing |

## Approach

Three shell concerns and two Markdown skills.

**Deployment.** A new `runner/deploy.sh` copies the runner from the plugin (this repository) to
`~/.local/share/autopilot/`. It is the one place that knows both halves of the version comparison:
its own `runner/VERSION` and the deployed `VERSION`. It refuses a destination that contains `.git`,
because overwriting the still-git checkout on this machine would destroy work. It copies and compares
version; a stale deployed copy is re-copied, and the result is printed so the skill can report it.

**Skills.** Two `SKILL.md` files. `autopilot-deliver` implements the six-phase flow from the design
and calls `deploy.sh` as part of prepare; `autopilot-review` reads the six sources and produces the
brief. Both live under `skills/`, which the boundary test already guarantees the loop never reaches.

**Packaging.** Add `skills/` to the plugin (the manifest needs no change — the plugin already carries
`runner/` beside it), and bump the version across `runner/VERSION`, `.claude-plugin/plugin.json`, and
`.claude-plugin/marketplace.json` because this is a new capability. `tests/test_plugin.sh` already
asserts the three versions agree.

### Files

- Create: `runner/deploy.sh`
- Create: `tests/test_deploy.sh`
- Create: `skills/autopilot-deliver/SKILL.md`
- Create: `skills/autopilot-review/SKILL.md`
- Modify: `runner/VERSION`, `.claude-plugin/plugin.json`, `.claude-plugin/marketplace.json`
- Modify: `README.md` (plugin now ships skills and a deploy step)

## Work breakdown

| Step | Output | Depends on |
|---|---|---|
| 1 | `runner/deploy.sh` — copy-out with `.git` refusal and stale-version reporting | — |
| 2 | `tests/test_deploy.sh` — refusal, fresh deploy, stale re-deploy | 1 |
| 3 | `skills/autopilot-deliver/SKILL.md` | 1 |
| 4 | `skills/autopilot-review/SKILL.md` | 1 |
| 5 | Bump version to `0.2.0` across the three places, update README | 1–4 |

## Verification

| Step | How it is proven |
|---|---|
| 2 | Unit tests against real throwaway directories. Refuse a destination with `.git` and name what it found; deploy into an empty path; re-deploy over a stale copy and confirm `VERSION` travels with it |
| 3, 4 | Skills are Markdown and cannot be unit tested. Stated plainly: they are verified by running end-to-end against a throwaway repository, which is what the shell suite already does |
| 5 | `sh tests/run.sh` — `test_plugin.sh` asserts one version across `plugin.json`, `marketplace.json`, and `runner/VERSION` |
| — | Boundary invariant: `grep -rl "skills/" runner/run-once.sh runner/lib/` stays empty, asserted by `tests/test_run_once.sh` |
| — | `shellcheck` clean on `runner/deploy.sh` |

## Documentation to update

- [ ] `README.md` — the plugin now ships skills and a deploy step
- [ ] `docs/guides/install.md` — the plugin route now actually deploys

## Risks

| Risk | Level | Mitigation |
|---|---|---|
| `deploy.sh` overwrites a real checkout on the developer machine | High | Refuse any destination containing `.git`, print what it found, exit non-zero |
| A skill runs while a scheduled run is in flight and rewrites config | Medium | The skill checks the lock `guard_lock` holds before touching config (per design) |
| The version bump desyncs and `test_plugin.sh` catches it late | Low | That test already runs in the suite before every commit |

## Out of scope

- Re-sharding after a plan changes. It requires reconciling against issues that already exist and may
  be closed — a different problem, named in the design and deliberately not solved here.
- Replacing the loop's supervision. `autopilot-review` reports and asks; it never closes a
  `needs-human` issue, removes `STOP`, or merges to `main` without being told.