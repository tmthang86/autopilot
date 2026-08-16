# The journal dashboard — implementation plan

> **Type:** Plan · **Date:** 2026-08-16 · **Status:** Awaiting approval
> **Prerequisite:** docs/plans/2026-08-16-planning-skill.md, docs/plans/2026-08-16-rust-runner.md
> **Scope:** Sub-project E of the multi-harness design — a read-only Go + htmx dashboard over the
> run journals, whose headline feature is showing a role that started and never finished.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make an unattended loop legible — what is running now, what died without saying so, what
each tier is costing, and what is paused waiting for a person — across every project on the machine.

**Architecture:** One Go binary outside `runner/`, serving server-rendered HTML with htmx for
partial refresh. It reads journals and `launchctl`; it never writes. The runner does not know it
exists, and that is a grep-checked test on both sides.

**Tech Stack:** Go with the standard library plus `html/template` and `embed`. htmx vendored as one
file. No JavaScript build step, no package manager, no external Go module.

**Spec:** [`docs/design/2026-08-16-multi-harness-role-pipeline-design.md`](../design/2026-08-16-multi-harness-role-pipeline-design.md)
— decisions 16 and 25, and the sections *The journal*, *The dashboard*, and *The boundary, restated
and widened*.

## Global Constraints

- **Read-only. Always.** No handler mutates anything, and no route accepts `POST`. Acting on what
  the dashboard shows happens through the skills or by hand.
- **The runner never references the dashboard**, and the dashboard is never invoked by the runner.
  Both directions are grep tests.
- **The journal is the only interface.** No shared library, no database, no IPC. If the dashboard
  needs a fact, the journal must record it — which is a change to plan 2, not a shortcut here.
- **Zero external Go modules.** `go.mod` lists no `require`. The whole point of a compiled binary
  outside the runner is that the operator can still read what it is.
- **`reported`, `estimated` and `unknown` costs are never summed into one figure.** Decision 25. A
  total that mixes them without saying so is this repository's recurring failure in a nicer font.
- `go vet ./...` and `gofmt -l` clean before every commit.
- **Localhost only, by default.** It binds `127.0.0.1` and refuses to bind anything else without an
  explicit flag, because a dashboard over an operator's projects is not something to serve by
  accident.

---

## Context

Every previous form of this repository's central failure had the same shape: *something did not work,
and the thing reporting on it said otherwise.* `ctl.sh start` discarding an exit status; a missing
label read as an empty queue; a launchd job that loads and never runs; `gh label create` failing for
a reason nobody read. The design adds nine agent calls per task, so the number of places that can die
quietly goes up by roughly an order of magnitude.

The journal is the answer to that, and it already answers it without any UI: a `role_start` with no
matching `role_end` is one `jq` query. This plan is the view on top — worth building because an
operator will run one query when prompted and none when not, and because cost per tier is the number
that decides whether the tier ladder was chosen well and nobody reads it out of JSONL.

The order matters and is deliberate: the journal shipped in plan 2 task 2. A dashboard with nothing
trustworthy behind it would be the disease, not the cure.

## What we already know

- **The journal format is fixed** by plan 2 task 2: JSONL at `.autopilot/runs/<issue>/journal.jsonl`,
  events `wake_start`, `role_start`, `role_end`, `verdict`, `verify`, `wake_end`, each with `ts` and
  `wake`. `role_end` carries `role`, `round`, `tier`, `lens`, `classify`, `cost_usd`, `cost_source`
  and `duration_s`.
- **Installed projects are already enumerable** from `~/.local/share/autopilot/jobs/*.plist`
  (ADR-0004). Each plist names its project path, so the dashboard needs no registry of its own.
- **`launchctl print gui/$UID/<label>` reports `runs` and `last exit code`**, and
  `observed-behaviour.md` names that as *"the check that would catch it"* for the `EX_CONFIG`
  failure. Plan 2 task 19 makes `autopilot status` read it; the dashboard reads the same source.
- **`78` is `EX_CONFIG`**, observed 2026-08-15 on a job that loaded, reported success, and never ran.
- **Only `claude` reports cost today.** `pi` and `opencode` are unmeasured; local models are zero.
  So a per-wake total is not a number, it is three numbers, and must be shown as three.

## Approach

Six tasks. The parser first — everything else is a view over it — then the boundary tests, then the
pages, then a real run.

**Files created** — `dashboard/go.mod`, `dashboard/main.go`, `dashboard/journal.go`,
`dashboard/projects.go`, `dashboard/render.go`, `dashboard/ui/*.html`, `dashboard/ui/htmx.min.js`,
`dashboard/*_test.go`, `dashboard/README.md`

**Files modified** — `AGENTS.md` (§1 boundary between runner and optional tooling), `README.md`,
`docs/guides/install.md`, `runner/tests/boundaries.rs` (the reverse-direction check)

---

## Work breakdown

### Task 1: The journal parser, and the orphan it exists to find

- **Done when:** a journal parses into typed events, a `role_start` with no `role_end` is reported as an orphan with its age, and a truncated final line is tolerated rather than fatal.
- **Verify:** `cd dashboard && go test ./... -run Journal` → ok, 9 tests
- **Intent:** `docs/design/2026-08-16-multi-harness-role-pipeline-design.md`
- **Depends on:** —
- **Tier:** standard
- **Needs human:** no

**Files:**
- Create: `dashboard/go.mod`, `dashboard/journal.go`, `dashboard/journal_test.go`

**Interfaces:**
- Produces:
  - `type Event struct { TS time.Time; Wake, Event, Role, Tier, Harness, Model, Lens, Classify string; Round int; CostUSD *float64; CostSource string; DurationS int64 }`
  - `func ParseJournal(r io.Reader) ([]Event, error)`
  - `func Orphans(evs []Event, now time.Time) []Orphan` — a `role_start` with no `role_end` sharing
    its `wake`, `role`, `round` and `lens`.
  - `func CostByTier(evs []Event) map[string]TierCost` where
    `TierCost { Reported float64; Estimated float64; UnknownCount int }` — **three fields, never
    one**.

- [ ] **Step 1: Write the failing tests**

```go
func TestAnOrphanedRoleIsFound(t *testing.T) {
    // The headline feature. A role that started and never reported back is the
    // failure the operator hit on 2026-08-16, and the reason this exists.
    in := `{"ts":"2026-08-16T01:00:00Z","wake":"w-1","event":"role_start","role":"review","round":1,"tier":"deep","lens":"partial-failure"}
{"ts":"2026-08-16T01:00:05Z","wake":"w-1","event":"role_start","role":"test","round":1,"tier":"light"}
{"ts":"2026-08-16T01:02:00Z","wake":"w-1","event":"role_end","role":"test","round":1,"tier":"light","classify":"ok","cost_usd":0.1,"cost_source":"reported","duration_s":115}`
    evs, err := ParseJournal(strings.NewReader(in))
    if err != nil { t.Fatal(err) }
    o := Orphans(evs, mustTime("2026-08-16T02:00:00Z"))
    if len(o) != 1 { t.Fatalf("want 1 orphan, got %d", len(o)) }
    if o[0].Role != "review" || o[0].Lens != "partial-failure" {
        t.Fatalf("wrong orphan: %+v", o[0])
    }
    if o[0].Age < time.Hour { t.Fatalf("age must be measured from role_start, got %v", o[0].Age) }
}

func TestATruncatedFinalLineIsNotFatal(t *testing.T) {
    // A journal is being appended to while it is read, and a process killed
    // mid-write leaves half a line. Refusing to parse would hide every event
    // before it -- turning the one artefact that survives a crash into nothing.
    in := `{"ts":"2026-08-16T01:00:00Z","wake":"w-1","event":"wake_start","issue":4,"tier":"light"}
{"ts":"2026-08-16T01:00:05Z","wake":"w-1","event":"role_st`
    evs, err := ParseJournal(strings.NewReader(in))
    if err != nil { t.Fatalf("a truncated tail must not fail the parse: %v", err) }
    if len(evs) != 1 { t.Fatalf("want the 1 complete event, got %d", len(evs)) }
}

func TestCostsOfDifferentProvenanceAreNeverSummed(t *testing.T) {
    // Decision 25. One figure mixing measured and unknown is worse than no
    // figure, because someone will act on it.
    in := `{"event":"role_end","tier":"deep","cost_usd":1.5,"cost_source":"reported"}
{"event":"role_end","tier":"deep","cost_usd":null,"cost_source":"unknown"}`
    evs, _ := ParseJournal(strings.NewReader(in))
    c := CostByTier(evs)["deep"]
    if c.Reported != 1.5 { t.Fatalf("reported: want 1.5, got %v", c.Reported) }
    if c.UnknownCount != 1 { t.Fatalf("unknown must be counted, not zeroed") }
}
```

- [ ] **Step 2: Run and confirm failure**

```sh
cd dashboard && go test ./... -run Journal
```

Expected: build failure — `ParseJournal` undefined.

- [ ] **Step 3: Implement**, decoding line by line and **discarding only the final line** if it
  fails to parse. A parse error on any earlier line is a real error and is returned.

- [ ] **Step 4: Run tests and vet**

```sh
cd dashboard && go test ./... && go vet ./... && gofmt -l .
```

Expected: `ok`, no vet output, `gofmt -l` prints nothing.

- [ ] **Step 5: Commit**

```sh
git add dashboard/go.mod dashboard/journal.go dashboard/journal_test.go
git commit -m "feat: parse run journals and find roles that never finished

A role_start with no role_end is the signature of the failure this whole
dashboard exists for, and it needs no UI to detect -- which is why the parser
lands before anything renders.

A truncated final line is tolerated. A journal is appended to while it is read,
and refusing to parse a half-written tail would hide every event before it,
turning the one artefact that survives a crash into nothing."
```

---

### Task 2: Project discovery and the two boundary tests

- **Done when:** installed projects are found from the job plists, and both directions of the runner/dashboard boundary are tests.
- **Verify:** `cd dashboard && go test ./... -run Boundary` → ok; `cd runner && cargo test --test boundaries` → still passes
- **Intent:** `docs/decisions/0004-job-files-live-on-the-internal-disk.md` `docs/design/2026-08-16-multi-harness-role-pipeline-design.md`
- **Depends on:** Task 1
- **Tier:** standard
- **Needs human:** no

**Files:**
- Create: `dashboard/projects.go`, `dashboard/boundary_test.go`

**Interfaces:**
- Produces: `func Projects(jobsDir string) ([]Project, error)` where
  `Project { Label, Path string; Loaded bool; Runs int; LastExit int }`, the last three read from
  `launchctl print gui/<uid>/<label>`.

- [ ] **Step 1: The boundary tests, both directions**

```go
func TestTheDashboardNeverWrites(t *testing.T) {
    // Read-only is the whole safety argument. os.Create, os.WriteFile, exec of
    // git or gh -- any of them turns a viewer into a second actor on the same
    // repositories the runner is working in.
    for _, f := range goFiles("..") {
        if strings.Contains(f.path, "_test.go") { continue }
        for _, bad := range []string{"os.Create", "os.WriteFile", "os.Remove", "os.OpenFile"} {
            if strings.Contains(f.body, bad) { t.Errorf("%s writes: %s", f.path, bad) }
        }
    }
}

func TestNoPostRoutes(t *testing.T) {
    for _, f := range goFiles("..") {
        if strings.Contains(f.body, "MethodPost") { t.Errorf("%s accepts POST", f.path) }
    }
}
```

- [ ] **Step 2: Add the reverse check to the Rust side**

`runner/tests/boundaries.rs` already asserts the runner never mentions the dashboard. Confirm it
covers the new name and leave a comment saying the pair is deliberate — one check on each side, so
neither can be deleted alone without the other failing.

- [ ] **Step 3: Implement `Projects`** by reading each plist's `ProgramArguments` for `--project`.
  A plist that cannot be parsed is reported as a project in an error state, never skipped: a project
  silently missing from the dashboard is the failure this repository keeps having.

- [ ] **Step 4–5: Run, commit.**

---

### Task 3: The overview page

- **Done when:** one page lists every project with its loaded state, `runs`, last exit code, whether a `STOP` file exists, wakes in flight, and orphaned roles at the top.
- **Verify:** `curl -s localhost:8787/ | grep -c 'orphan'` → at least 1 when a fixture journal has one; `go test ./... -run Render` → ok
- **Intent:** `docs/design/2026-08-16-multi-harness-role-pipeline-design.md` `docs/reference/observed-behaviour.md`
- **Depends on:** Task 2
- **Tier:** standard
- **Needs human:** no

**Files:**
- Create: `dashboard/main.go`, `dashboard/render.go`, `dashboard/ui/overview.html`,
  `dashboard/ui/htmx.min.js`

- [ ] **Step 1: Decide the page's order of attention, and encode it as a test**

```go
func TestOrphansRenderAboveEverythingElse(t *testing.T) {
    // Ordering is the design. A dashboard that puts a cost chart above a role
    // that has been dead for three hours has buried the only urgent thing.
    html := render(fixtureWithOrphanAndCosts())
    if strings.Index(html, "orphan") > strings.Index(html, "cost-by-tier") {
        t.Fatal("orphans must render before costs")
    }
}

func TestALoadedJobWithZeroRunsIsCalledOut(t *testing.T) {
    // EX_CONFIG, observed 2026-08-15: loaded: yes and autopilot started were
    // both true and both useless. runs = 0 well past the interval is the tell.
    html := render(project(Loaded: true, Runs: 0, LastExit: 78))
    if !strings.Contains(html, "loaded but has never run") {
        t.Fatal("a job that loads and never runs must be named, not shown as healthy")
    }
}
```

- [ ] **Step 2: Write the template**, with htmx polling only the fragment that changes:

```html
<div id="live" hx-get="/fragment/live" hx-trigger="every 10s" hx-swap="outerHTML">
```

- [ ] **Step 3: Serve `127.0.0.1` by default**, with `--addr` required to bind anything else and a
  startup line stating what it bound.

- [ ] **Step 4: Embed the assets** with `//go:embed ui`, so the binary is the whole program.

- [ ] **Step 5–6: Run against a fixture directory, commit.**

---

### Task 4: The task view — the argument, the rounds, and the cost

- **Done when:** clicking an issue shows its wakes, each role and lens in order with its verdict and reason, verify results, and cost split three ways by provenance.
- **Verify:** `go test ./... -run TaskView` → ok, 6 tests including one asserting the three cost figures are never combined
- **Intent:** `docs/design/2026-08-16-multi-harness-role-pipeline-design.md`
- **Depends on:** Task 3
- **Tier:** standard
- **Needs human:** no

- [ ] **Step 1: The test for the thing that is easy to get wrong**

```go
func TestTheThreeCostFiguresAreShownSeparately(t *testing.T) {
    html := renderTask(fixtureMixedProvenance())
    for _, want := range []string{"reported", "estimated", "unknown"} {
        if !strings.Contains(html, want) { t.Errorf("cost provenance %q not shown", want) }
    }
}

func TestARejectedVerdictShowsItsReasonInFull(t *testing.T) {
    // The reason is why a task paused. Truncating it makes the page pretty and
    // the coordinator's morning longer.
    html := renderTask(fixtureWithLongRejection())
    if strings.Contains(html, "…") { t.Fatal("a verdict reason must not be truncated") }
}
```

- [ ] **Step 2: Render the round structure** so the tester↔reviewer argument reads in order, with the
  lens named on each review call.

- [ ] **Step 3–4: Run, commit.**

---

### Task 5: Deployment, and keeping it out of the runner's way

- **Done when:** `go build` produces one binary, `docs/guides/install.md` explains that it is optional and read-only, and `AGENTS.md` §1 states the boundary.
- **Verify:** `cd dashboard && go build -o /tmp/apdash && /tmp/apdash --help` → usage including `--addr`; `grep -c 'read-only' AGENTS.md` → at least 1
- **Intent:** `docs/design/2026-08-16-multi-harness-role-pipeline-design.md` `AGENTS.md`
- **Depends on:** Task 4
- **Tier:** light
- **Needs human:** no

- [ ] **Step 1: Amend `AGENTS.md` §1** with the sentence the design requires:

> The runner runs unsupervised and obeys §1 absolutely. The dashboard is an **optional tool that
> runs only when a person is present**, is read-only, and the runner never knows it exists. Its
> dependency budget is separate and its failure cannot affect a scheduled run.

- [ ] **Step 2: Write `dashboard/README.md`** — what it shows, what it deliberately cannot do, and
  the one `jq` query that finds an orphan without it. That query is the honest fallback and stating
  it prevents the dashboard becoming load-bearing.

- [ ] **Step 3–4: Build, commit.**

---

### Task 6: Watch a real overnight run through it

- **Done when:** the dashboard has been left open across a real scheduled run on a real project, and what it showed matched what actually happened.
- **Verify:** after an overnight run, the dashboard's orphan list, cost-by-tier and paused tasks each agree with `git log autopilot/main`, the issues, and the journals read directly
- **Intent:** `docs/design/2026-08-16-multi-harness-role-pipeline-design.md` `docs/reference/observed-behaviour.md`
- **Depends on:** Task 5
- **Tier:** deep
- **Needs human:** yes

`Needs human: yes` — the question is whether the page told the operator something true and useful
overnight, and no test answers that.

- [ ] **Step 1: Leave it running across a real scheduled night.**

- [ ] **Step 2: In the morning, check it against the ground truth *before* believing it**

Read the journals, the issues and `git log` directly, then compare. A dashboard is a claim about a
system, and this repository's whole history is claims about systems that were wrong while looking
right.

- [ ] **Step 3: Kill a role mid-run deliberately, and confirm the orphan appears**

The headline feature must be demonstrated, not assumed. Send `SIGKILL` to an agent process during a
wake and confirm the page names it, with its age and its lens.

- [ ] **Step 4: Record what it got wrong** in `docs/reference/observed-behaviour.md`, with the date.
  If it got nothing wrong, record what was run and observed, so the claim is testable rather than
  asserted.

---

## Verification

| Check | Command | Passing looks like |
|---|---|---|
| Unit | `cd dashboard && go test ./...` | `ok`, ~25 tests |
| Vet and format | `go vet ./... && gofmt -l .` | no output |
| Read-only | Task 2's boundary tests | no writes, no POST routes |
| No modules | `grep -c require dashboard/go.mod` | `0` |
| Runner unaware | `cd runner && cargo test --test boundaries` | passes |
| Exercised for real | Task 6 | an orphan deliberately created and correctly shown |

## Documentation to update

- [ ] `AGENTS.md` §1 — the boundary between the runner and optional tooling (Task 5)
- [ ] `README.md` — the dashboard exists, is optional and read-only (Task 5)
- [ ] `docs/guides/install.md` — how to build and run it (Task 5)
- [ ] `dashboard/README.md` — what it shows, and the `jq` fallback (Task 5)
- [ ] `docs/reference/observed-behaviour.md` — the overnight run (Task 6)
- [ ] `docs/README.md` status table — this plan's row (Task 6)

## Risks

| Risk | Level | Mitigation |
|---|---|---|
| The dashboard becomes load-bearing, and a missing page reads as a healthy night | **High** | It is read-only, optional, and `dashboard/README.md` states the `jq` query that answers the same question without it. `autopilot status` reports orphans too — the dashboard is never the only route |
| A summed cost figure mixing measured and unknown misleads a spending decision | **High** | Three separate figures, asserted by tests in Tasks 1 and 4. Never one number |
| Someone adds a write action "just to remove a STOP file" | Medium | A test fails on `os.Create`, `os.WriteFile` and `MethodPost`. Removing `STOP` is a decision and §2.4 reserves decisions for a person acting deliberately |
| Reading a journal being written produces a parse error and hides everything | Medium | Task 1 tolerates a truncated final line and nothing else, with a test |
| It gets served beyond localhost by accident | Medium | Binds `127.0.0.1` unless `--addr` is passed, and prints what it bound at startup |
| Go is a third language in a Rust and Markdown repository | Low | Accepted in decision 16. It is outside `runner/`, has no modules, and its failure cannot reach a scheduled run |

## Out of scope

- **Any write action.** No removing `STOP`, no answering a blocked issue, no merging. Those are
  `autopilot-review`'s and the operator's.
- **Authentication, TLS, multi-user.** Localhost, one operator, one machine.
- **Historical analytics beyond what the journals hold.** No database, no retention policy, no
  aggregation across machines.
- **Replacing `autopilot status`.** The CLI stays the primary and scriptable route; the dashboard is
  a second view over the same facts, never the only one.
- **Live control of a running wake.** Read-only means read-only.
