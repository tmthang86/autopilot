# The journal dashboard — implementation plan

> **Type:** Plan · **Date:** 2026-08-16 · **Status:** Awaiting approval · **Rewritten 2026-08-19 against decisions 26–42**
> **Prerequisite:** docs/plans/2026-08-16-planning-skill.md, docs/plans/2026-08-16-rust-runner.md
> **Scope:** Sub-project E of the multi-harness design — a read-only Go + htmx dashboard over the run
> journals, whose headline feature is showing a role that started and never finished.

> **For agentic workers:** Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make an unattended loop legible — what is running now, what died without saying so, what
each tier is costing, and what is paused waiting for a person — across every project on the machine.

**Architecture:** One Go binary outside `runner/`, serving server-rendered HTML with htmx for
partial refresh. It reads the single per-project journal (`.autopilot/journal.jsonl`, decision 30)
and `launchctl`; it never writes. The runner does not know it exists, and that is a grep-checked test
on both sides.

**Tech Stack:** Go with the standard library plus `html/template` and `embed`. htmx vendored as one
file. No JavaScript build step, no package manager, no external Go module.

**Spec:** [`docs/design/2026-08-16-multi-harness-role-pipeline-design.md`](../design/2026-08-16-multi-harness-role-pipeline-design.md)
— decisions 16, 25, 30, 31, 40 and 42, and the sections *The journal*, *The dashboard*, and *The
boundary, restated and widened*.

## Global Constraints

- **Read-only. Always.** No handler mutates anything, and no route accepts `POST`.
- **The runner never references the dashboard**, and the dashboard is never invoked by the runner.
  Both directions are grep tests.
- **The journal is the only interface.** No shared library, no database, no IPC.
- **Zero external Go modules.** `go.mod` lists no `require`.
- **`reported` and `unknown` costs are never summed into one figure.** Decision 25/31. A total that
  mixes them without saying so is this repository's recurring failure in a nicer font.
- `go vet ./...` and `gofmt -l` clean before every commit.
- **Localhost only, by default.** It binds `127.0.0.1` and refuses to bind anything else without an
  explicit flag.

## Context

Every previous form of this repository's central failure had the same shape: *something did not work,
and the thing reporting on it said otherwise.* The design adds up to nine agent calls per task, so
the number of places that can die quietly goes up by roughly an order of magnitude. The journal is
the answer, and a `role_start` with no matching `role_end` is the signature the whole dashboard
exists to surface. Decision 26 had deferred this plan behind a cost measurement; **decision 42
authorises building it now**.

## What we already know

- **The journal format is fixed** by plan 1 task 2: one JSONL file per project at
  `.autopilot/journal.jsonl`, events `wake_start`, `role_start`, `role_end`, `verdict`, `verify`,
  `wake_end`, each with `ts` and `wake`. `wake_start` carries a nullable `issue` (decision 30).
  `role_end` carries `role`, `round`, `tier`, `lens`, `classify`, `cost_usd`, `cost_source`, and
  `duration_s`. `cost_source` is `reported` or `unknown` (decision 31).
- **Installed projects are already enumerable** from `~/.local/share/autopilot/jobs/*.plist`
  (ADR-0004). Each plist names its project path.
- **`launchctl print gui/$UID/<label>` reports `runs` and `last exit code`**, and `78` is `EX_CONFIG`
  (observed 2026-08-15).
- **Only `claude` and `pi` report cost today**; `opencode` is unmeasured. A per-wake total is
  therefore not one number.

## Approach

Five tasks. The parser first — everything else is a view over it — then the boundary tests, then the
pages, then a real run.

**Files created** — `dashboard/go.mod`, `dashboard/main.go`, `dashboard/journal.go`,
`dashboard/projects.go`, `dashboard/render.go`, `dashboard/ui/*.html`, `dashboard/ui/htmx.min.js`,
`dashboard/*_test.go`, `dashboard/README.md`

**Files modified** — `CLAUDE.md` (§1 boundary between runner and optional tooling), `README.md`,
`docs/guides/install.md`, `runner/tests/boundaries.rs` (the reverse-direction check)

---

## Work breakdown

### Task 1: The journal parser, and the orphan it exists to find

- **Done when:** a journal parses into typed events; a `role_start` with no `role_end` is reported as
  an orphan with its age; a truncated final line is tolerated rather than fatal; reported and unknown
  costs are kept apart.
- **Verify:** `cd dashboard && go test ./... -run Journal` → ok, 9 tests
- **Intent:** docs/design/2026-08-16-multi-harness-role-pipeline-design.md
- **Depends on:** —
- **Tier:** standard
- **Needs human:** no

**Files:**
- Create: `dashboard/go.mod`, `dashboard/journal.go`, `dashboard/journal_test.go`

**Interfaces:**
- `type Event struct { TS time.Time; Wake, Event, Role, Tier, Harness, Model, Lens, Classify string; Round int; CostUSD *float64; CostSource string; DurationS int64 }`
- `func ParseJournal(r io.Reader) ([]Event, error)` — decodes line by line and discards only a
  truncated final line.
- `func Orphans(evs []Event, now time.Time) []Orphan`
- `func CostByTier(evs []Event) map[string]TierCost` where `TierCost { Reported float64; UnknownCount int }`.

- [ ] **Step 1: Write the failing tests** — an orphaned role is found with its age; a truncated final
  line is not fatal; reported and unknown costs are never summed.
- [ ] **Step 2: Run and confirm failure.**
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run `go test`, `go vet`, `gofmt -l`.**
- [ ] **Step 5: Commit.**

### Task 2: Project discovery and the two boundary tests

- **Done when:** installed projects are found from the job plists, and both directions of the
  runner/dashboard boundary are tests.
- **Verify:** `cd dashboard && go test ./... -run Boundary` → ok; `cd runner && cargo test --test boundaries` → still passes
- **Intent:** docs/decisions/0004-job-files-live-on-the-internal-disk.md docs/design/2026-08-16-multi-harness-role-pipeline-design.md
- **Depends on:** Task 1
- **Tier:** standard
- **Needs human:** no

**Files:**
- Create: `dashboard/projects.go`, `dashboard/boundary_test.go`

**Interfaces:**
- `func Projects(jobsDir string) ([]Project, error)` where `Project { Label, Path string; Loaded bool; Runs int; LastExit int }`.

- [ ] **Step 1: Write both boundary tests** — the dashboard never writes (no `os.Create`,
  `os.WriteFile`, `os.Remove`, `os.OpenFile` outside tests); no `MethodPost` routes; and confirm the
  Rust side's reverse check still passes.
- [ ] **Step 2: Run and confirm failure.**
- [ ] **Step 3: Implement `Projects`** by reading each plist's `ProgramArguments` for `--project`. A
  plist that cannot be parsed is reported in an error state, never skipped.
- [ ] **Step 4: Run tests.**
- [ ] **Step 5: Commit.**

### Task 3: The overview page

- **Done when:** one page lists every project with its loaded state, `runs`, last exit code, whether a
  `STOP` file exists, wakes in flight, and orphaned roles at the top.
- **Verify:** `go test ./... -run Render` → ok; a fixture journal with one orphan renders `orphan` before `cost-by-tier`
- **Intent:** docs/design/2026-08-16-multi-harness-role-pipeline-design.md docs/reference/observed-behaviour.md
- **Depends on:** Task 2
- **Tier:** standard
- **Needs human:** no

**Files:**
- Create: `dashboard/main.go`, `dashboard/render.go`, `dashboard/ui/overview.html`, `dashboard/ui/htmx.min.js`

**Interfaces:**
- The page's order of attention is encoded as a test: orphans render above everything else; a loaded
  job with zero runs is called out (`EX_CONFIG`). htmx polls only the live fragment. Serves
  `127.0.0.1` by default with `--addr` to bind elsewhere.

- [ ] **Step 1: Write the ordering tests.**
- [ ] **Step 2: Run and confirm failure.**
- [ ] **Step 3: Write the template, the htmx fragment, and the embedded assets.**
- [ ] **Step 4: Run against a fixture directory.**
- [ ] **Step 5: Commit.**

### Task 4: The task view — the argument, the rounds, and the cost

- **Done when:** clicking an issue shows its wakes, each role and lens in order with its verdict and
  reason, verify results, and cost split by provenance.
- **Verify:** `go test ./... -run TaskView` → ok, 6 tests including one asserting the two cost figures are never combined
- **Intent:** docs/design/2026-08-16-multi-harness-role-pipeline-design.md
- **Depends on:** Task 3
- **Tier:** standard
- **Needs human:** no

**Files:**
- Create: `dashboard/ui/task.html`
- Modify: `dashboard/render.go`

**Interfaces:**
- The two cost figures (`reported`, `unknown`) are shown separately; a rejected verdict's reason is
  shown in full, never truncated.

- [ ] **Step 1: Write the tests for the two things that are easy to get wrong.**
- [ ] **Step 2: Run and confirm failure.**
- [ ] **Step 3: Render the round structure** so the tester↔reviewer argument reads in order with each
  lens named.
- [ ] **Step 4: Run tests.**
- [ ] **Step 5: Commit.**

### Task 5: Deployment, and keeping it out of the runner's way

- **Done when:** `go build` produces one binary; `docs/guides/install.md` explains that it is optional
  and read-only; `CLAUDE.md` §1 states the boundary; `dashboard/README.md` documents the `jq` fallback.
- **Verify:** `cd dashboard && go build -o /tmp/apdash && /tmp/apdash --help` → usage including `--addr`; `grep -c 'read-only' CLAUDE.md` → at least 1
- **Intent:** docs/design/2026-08-16-multi-harness-role-pipeline-design.md CLAUDE.md
- **Depends on:** Task 4
- **Tier:** light
- **Needs human:** no

**Files:**
- Modify: `CLAUDE.md`, `README.md`, `docs/guides/install.md`
- Create: `dashboard/README.md`

**Interfaces:**
- `CLAUDE.md` §1 gains: the dashboard is an optional tool that runs only when a person is present,
  is read-only, and the runner never knows it exists.

- [ ] **Step 1: Amend `CLAUDE.md` §1.**
- [ ] **Step 2: Write `dashboard/README.md`**, including the one `jq` query that finds an orphan
  without it — the honest fallback.
- [ ] **Step 3: Build and commit.**

---

## Verification

| Check | Command | Passing looks like |
|---|---|---|
| Unit | `cd dashboard && go test ./...` | `ok`, ~25 tests |
| Vet and format | `go vet ./... && gofmt -l .` | no output |
| Read-only | Task 2's boundary tests | no writes, no POST routes |
| No modules | `grep -c require dashboard/go.mod` | `0` |
| Runner unaware | `cd runner && cargo test --test boundaries` | passes |
| Exercised for real | a real running process serving fixture journals | an orphan deliberately created and correctly shown |

## Documentation to update

- [ ] `CLAUDE.md` §1 — the boundary between the runner and optional tooling (Task 5)
- [ ] `README.md` — the dashboard exists, is optional and read-only (Task 5)
- [ ] `docs/guides/install.md` — how to build and run it (Task 5)
- [ ] `dashboard/README.md` — what it shows, and the `jq` fallback (Task 5)
- [ ] `docs/reference/observed-behaviour.md` — anything learned while exercising it

## Risks

| Risk | Level | Mitigation |
|---|---|---|
| The dashboard becomes load-bearing, and a missing page reads as a healthy night | **High** | Read-only, optional, and `dashboard/README.md` states the `jq` query that answers the same question without it. `autopilot status` reports orphans too |
| A summed cost figure mixing measured and unknown misleads a spending decision | **High** | Separate figures, asserted by tests in Tasks 1 and 4 |
| Someone adds a write action "just to remove a STOP file" | Medium | A test fails on `os.Create`, `os.WriteFile` and `MethodPost` |
| Reading a journal being written produces a parse error and hides everything | Medium | Task 1 tolerates a truncated final line and nothing else, with a test |
| It gets served beyond localhost by accident | Medium | Binds `127.0.0.1` unless `--addr` is passed, and prints what it bound at startup |

## Out of scope

- **Any write action.** No removing `STOP`, no answering a blocked issue, no merging.
- **Authentication, TLS, multi-user.** Localhost, one operator, one machine.
- **Historical analytics beyond what the journals hold.**
- **Replacing `autopilot status`.** The CLI stays the primary and scriptable route.
- **Live control of a running wake.** Read-only means read-only.
