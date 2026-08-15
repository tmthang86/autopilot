# Project bootstrap — the runner prepares its own queue

> **Type:** Plan · **Date:** 2026-08-15 · **Status:** **Abandoned** — never approved, superseded the
> same day by [the skill layer design](../design/2026-08-15-skill-layer-design.md)
> **Scope:** `install-project.sh`, `ctl.sh`, and a new `shard.sh`. The unattended loop is unchanged.

**Why it was abandoned.** This plan put the sharding of a plan into issues in a shell script,
`shard.sh`, reviewed by editing a JSON file. The design that replaced it puts sharding in a skill
reviewed in conversation, which is both a better review surface and a smaller shell codebase. The
label creation, the job-label collision fix, and the silent empty-queue fix specified below survive
unchanged and are carried forward by that design. Kept rather than deleted because the reasoning in
*What we already know* is still the evidence those three fixes rest on.

## Context

`CLAUDE.md` §3 says portability is the product. Today it is not true.

The runner can only be pointed at a repository that somebody already prepared by hand: labels
created one by one, and every task written out as an issue. `docs/guides/install.md:8-9` states this
as a precondition — *"its work must already exist as issues carrying the `autopilot` label"* — which
means the single largest piece of adoption work is the one piece the tool does not help with.

The consequence is measurable. This runner has been installed on exactly one repository, and that
repository has its labels and its eight M0 issues only because a person made them in an earlier
session. A second project today would install cleanly, start cleanly, and then report an empty queue
forever with nothing explaining why.

This plan makes the runner able to prepare a project: create the labels it depends on, and turn an
approved plan into a reviewed set of issues.

## What we already know

Verified by reading the code on 2026-08-15:

| Fact | Evidence |
|---|---|
| The installer creates no labels | `install-project.sh` writes config, `.gitignore` entries, and a plist. Nothing else |
| A repository without the label fails silently | `lib/queue.sh:28-30` ends in `2>/dev/null \|\| printf '[]'`, so a `gh` failure is indistinguishable from an empty queue |
| The job label collides across projects with the same directory name | `install-project.sh:26` builds `com.autopilot.$(basename "$PROJECT")`. `~/work/api` and `~/side/api` produce one label, and the second install overwrites the first's plist |
| The runner already knows how to drive an agent | `lib/agent.sh:38-46` — prompt on stdin, `stream-json` captured to a log |
| The owner/name of a repository is already derivable | `lib/queue.sh:12-24` — `queue_init()` parses it from the `origin` remote |
| A good target shape exists to copy | The eight M0 issues in the consuming project: goal, done-when checklist, verify block, prose citations of the governing documents |
| Two files must agree on the job path | ADR-0003's consequences already record that `install-project.sh` and `ctl.sh` are "both wrong together or right together" |

## Approach

Three changes. The first two are mechanical; the third is the substance.

### 1. Create the labels — `install-project.sh`

Seven `gh label create` calls, each `|| true` so re-running the installer is safe: `autopilot`,
`needs-human`, `blocked`, `status:in-progress`, `model:sonnet`, `model:opus`, `effort:low`,
`effort:medium`, `effort:high`. Names and colours come from the consuming project, which already
proved them.

The installer reports what it created. A label that already existed is not an error.

### 2. Make the job label unique — `install-project.sh`, `ctl.sh`

The label becomes `com.autopilot.<owner>-<repo>`, derived from the `origin` remote by the same
parsing `queue_init()` already does. The derivation moves into `lib/` so both scripts call one
function rather than each building the string themselves — the failure ADR-0003 anticipated.

Projects with no remote keep a basename-derived label plus a short hash of the absolute path, so
they remain distinct.

### 3. `shard.sh` — an approved plan becomes reviewed issues

A new top-level command, alongside `install-project.sh` and `ctl.sh`. It runs in the foreground,
with the operator present, and **never from the launchd job**.

```sh
sh shard.sh propose <project> <plan-path>    # writes .autopilot/proposed-issues.json
sh shard.sh create  <project>                # creates them on GitHub
```

**`propose`** invokes `claude` with a shard prompt and the plan file, and gets back a JSON array of
proposed issues: title, body, labels, the `Intent:` paths, and dependencies expressed **by index**
rather than by issue number, since no numbers exist yet. The result is written to
`.autopilot/proposed-issues.json` and nothing is created.

**`create`** reads that file, calls `gh issue create` for each entry in dependency order, and
rewrites each index reference into the real `Depends on #N` once the referenced issue has a number.

The two commands are separate on purpose. The gap between them is where the operator reads the file,
edits it, deletes what is wrong, and decides which tasks carry `needs-human`. **That review is the
human signature** the whole design depends on: an issue names the plan it serves, and that naming
means something only because a person put it there. One command that went straight from plan to
open issues would make the runner author the intent it later executes, which `CLAUDE.md` §2
invariant 4 forbids.

The shard agent runs **without** `bypassPermissions`. It reads one plan file and writes one JSON
file; it has no reason to hold the authority the delivery agent holds.

**Files changed:** `install-project.sh`, `ctl.sh`, new `shard.sh`, new `lib/label.sh`, new
`templates/shard-prompt.tmpl`, `tests/test_install.sh`, new `tests/test_shard.sh`,
new `tests/test_label.sh`.

## Work breakdown

| Step | Output | Depends on |
|---|---|---|
| 1 | ADR-0006 — the runner may prepare a queue, and why that does not breach invariant 4. Records the operator-present boundary as the deciding line | — |
| 2 | `lib/label.sh` with the shared derivation; `install-project.sh` and `ctl.sh` both switched to it | 1 |
| 3 | Label creation in `install-project.sh`, idempotent | 1 |
| 4 | `queue_candidates()` distinguishes "label missing" from "queue empty" — the silent failure this plan exists to end | 1 |
| 5 | `templates/shard-prompt.tmpl` and `shard.sh propose` | 1 |
| 6 | `shard.sh create`, including index-to-number rewriting in dependency order | 5 |
| 7 | Docs: `docs/guides/install.md` gains a bootstrap section; `CLAUDE.md` §3 notes the shard command is operator-run | 2–6 |

## Verification

| Step | How it is proven |
|---|---|
| 2 | Install two throwaway repositories with the same directory name; confirm two distinct plists and that `ctl.sh status` finds each. This is the current bug, so the test must fail before the fix |
| 3 | Install into a fresh repository with no labels; confirm all nine exist. Run the installer twice and confirm the second run is clean |
| 4 | Delete the `autopilot` label from a test repository and run the loop; confirm the log distinguishes it from an empty queue |
| 5 | Run `propose` against the consuming project's own approved design plan; read the JSON and judge whether a person would accept the decomposition. **This step is judged by reading it, not by an assertion** |
| 6 | `create` into a throwaway repository from a three-issue proposal where the third depends on the first; confirm real numbers in `Depends on #N` and that `queue_pick()` selects only the unblocked one |
| 7 | A second project taken from empty to a running loop using only the guide, with nothing typed that the guide does not state |

**Exercised against a real repository:** the end-to-end run in step 7 is the point of this plan. It
is not complete until a repository that is not the consuming project has been bootstrapped and has
closed one issue.

## Documentation to update

- [ ] `docs/decisions/0006-runner-prepares-its-own-queue.md` — new ADR
- [ ] `docs/guides/install.md` — bootstrap section; remove the precondition that issues already exist
- [ ] `CLAUDE.md` §3 — portability now includes queue preparation
- [ ] `docs/reference/observed-behaviour.md` — anything learned about `gh label create` or `gh issue create` ordering

## Risks

| Risk | Level | Mitigation |
|---|---|---|
| The operator stops reading `proposed-issues.json` and runs `create` reflexively, turning the signature into a rubber stamp | **High** | Accepted and named rather than engineered away — no mechanism can force attention. `create` prints the count and the titles and requires the file to have been modified at least once, so an untouched proposal is at minimum noticed |
| A sharded issue is too large, and the agent silently reinterprets its scope at execution time | High | The shard prompt is told to prefer more, smaller issues, and `needs-human` is applied whenever no command can prove the result. The review step is where this is actually caught |
| `create` fails partway and leaves a half-built queue with dangling index references | Medium | Create in dependency order and record each created number back into the JSON as it goes, so a re-run resumes rather than duplicates |
| The shard agent invents work the plan does not contain | Medium | The prompt forbids it explicitly and the plan text is the only input. This is what the review catches, and it is why the two commands are separate |
| Nine labels hardcoded in a shell script drift from what `config.json` says | Low | The queue labels are read from config where they already exist; only the fixed taxonomy is hardcoded |

## Out of scope

- **Sharding from inside the launchd loop.** The operator being present is the property that makes
  this safe, and a scheduled run has no operator.
- **Editing or approving the plan itself.** `shard.sh` reads a plan and never writes one.
- **Re-sharding after a plan changes.** Useful, and a separate problem: it requires reconciling
  against issues that already exist and may be closed.
