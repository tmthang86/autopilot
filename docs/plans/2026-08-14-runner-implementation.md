# Autopilot Runner Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build an unattended runner that takes one task from a queue per invocation, drives a coding agent to implement it, gates the commit on the project's own verification commands, and exits — surviving reboot and usage-window exhaustion without supervision.

**Architecture:** A POSIX shell entry point (`run-once.sh`) sources six single-responsibility libraries and orchestrates them in a fixed order: guards → select → claim → work → verify → settle. No long-lived process; all state lives in `<project>/.autopilot/state.json`. The OS scheduler (`launchd`) is the only persistent component. Everything project-specific is read from a committed config file, so `lib/` never names a language or build tool.

**Tech Stack:** POSIX shell, `jq` 1.7.1 for all JSON, `gh` 2.97.0 for the queue, `git`, the `claude` CLI in headless mode. Tests are plain shell against real throwaway git repositories with `gh` and `claude` stubbed on `PATH`.

**Spec:** [`the companion project/docs/plans/2026-08-14-autopilot-delivery.md`](https://github.com/tmthang86/the companion project/blob/main/docs/plans/2026-08-14-autopilot-delivery.md) — the design this plan implements. Also [`docs/decisions/0001-one-task-per-wake-over-persistent-daemon.md`](../decisions/0001-one-task-per-wake-over-persistent-daemon.md).

## Global Constraints

Every task's requirements implicitly include this section.

- **Shell is bash 3.2.57 or POSIX `sh`.** Verified on the target machine 2026-08-14. **No associative arrays (`declare -A`), no `mapfile`/`readarray`, no `${var^^}`, no `[[ ... =~ ]]` back-references.** These are bash 4+ and will fail silently or loudly on macOS. Write for `/bin/sh`.
- **Runtime dependencies are exactly `git`, `gh`, `jq`, and `claude`.** Adding another is a plan revision, not an implementation detail.
- **Config format is JSON, not YAML.** The design spec showed YAML; `yq` is not installed and shell YAML parsing is not acceptable in code that runs with elevated permissions. JSON costs nothing because `jq` is already required. *This is a deliberate, flagged deviation from the spec — see "Deviations" below.*
- **No file in `lib/` exceeds 150 lines.** Per [CLAUDE.md](../../CLAUDE.md) §1.
- **Every error path must preserve the four invariants** in [CLAUDE.md](../../CLAUDE.md) §2. A test that only exercises the happy path does not count as covering an invariant.
- **`set -eu` at the top of every script.** Not `set -o pipefail` — unavailable in strict POSIX `sh` and unreliable in bash 3.2 pipelines used here.
- **Never invent a JSON shape.** Fixtures in `tests/fixtures/` are recorded from real `gh` and `claude` output. If a shape is unknown, record it first.

## Deviations from the spec

| Spec said | Plan does | Why |
|---|---|---|
| `.autopilot/config.yml` | `.autopilot/config.json` | `yq` not installed; parsing YAML in shell is unacceptable in privileged code; `jq` already required |
| `lib/{guards,queue,agent,verify,settle}.sh` | adds `lib/config.sh`, `lib/state.sh`, `lib/log.sh` | The spec's five files would each exceed the 150-line limit; config loading and state round-tripping are separate responsibilities with separate tests |

Both are recorded here rather than made silently. If either is unacceptable, the spec is the authority and this plan changes.

## File structure

| File | Responsibility |
|---|---|
| `run-once.sh` | Entry point. Parses `--project`, sources libs, runs the fixed phase order, sets exit code. |
| `lib/log.sh` | Timestamped lines to stderr and to the run log. Nothing else. |
| `lib/config.sh` | Read and validate `config.json`; expose values through `cfg_get`. Fails loudly on a missing required key. |
| `lib/state.sh` | Read and write `state.json`. Must round-trip — invariant 3. |
| `lib/guards.sh` | Six independent checks that each answer "should this run proceed". No side effects except the lock. |
| `lib/queue.sh` | List candidate issues, resolve `Depends on #N`, claim and release. The only file that knows `gh` exists. |
| `lib/agent.sh` | Build the `claude` argument list, run it, capture the stream, and classify the outcome — invariant 2. |
| `lib/verify.sh` | Run the project's verify commands in order, stop at the first failure, return its output. Invariant 1. |
| `lib/settle.sh` | Commit and push and close, or reset and report. The only file that writes to git history. |
| `install-project.sh` | Create `.autopilot/` in a target repo and install its launchd job. |
| `templates/` | `config.json`, `prompt.tmpl`, `launchd.plist.tmpl` |
| `tests/harness.sh` | Assertions, temp-repo builder, `gh`/`claude` stubs. |
| `tests/test_<lib>.sh` | One per lib. |
| `tests/run.sh` | Runs every `test_*.sh`, prints a summary, exits non-zero on any failure. |

---

### Task 1: Test harness and logging

**Files:**
- Create: `tests/harness.sh`, `tests/run.sh`, `lib/log.sh`, `tests/test_log.sh`

**Interfaces:**
- Consumes: nothing
- Produces: `assert_eq expected actual name`, `assert_contains haystack needle name`, `assert_status expected_code name` (reads `$?` captured by caller), `make_repo` (prints path to a fresh git repo), `stub_bin name script_body` (writes an executable onto a stubbed `PATH`), `finish` (prints summary, exits 1 if any failure). From `lib/log.sh`: `log_info msg`, `log_warn msg`, `log_error msg`.

- [ ] **Step 1: Write the harness**

Create `tests/harness.sh`:

```sh
#!/bin/sh
# Test harness. Source this at the top of every tests/test_*.sh.
set -u

TESTS_RUN=0
TESTS_FAILED=0
REPO_ROOT=$(cd "$(dirname "$0")/.." && pwd)
export REPO_ROOT

# Every test gets its own scratch dir, removed on exit.
TEST_TMP=$(mktemp -d "${TMPDIR:-/tmp}/autopilot-test.XXXXXX")
export TEST_TMP
trap 'rm -rf "$TEST_TMP"' EXIT

# Stubs live here and take precedence over real binaries.
STUB_BIN="$TEST_TMP/stub-bin"
mkdir -p "$STUB_BIN"
PATH="$STUB_BIN:$PATH"
export PATH

assert_eq() {
    TESTS_RUN=$((TESTS_RUN + 1))
    if [ "$1" = "$2" ]; then
        printf '  ok   %s\n' "$3"
    else
        TESTS_FAILED=$((TESTS_FAILED + 1))
        printf '  FAIL %s\n       expected: [%s]\n       actual:   [%s]\n' "$3" "$1" "$2"
    fi
}

assert_contains() {
    TESTS_RUN=$((TESTS_RUN + 1))
    case "$1" in
        *"$2"*) printf '  ok   %s\n' "$3" ;;
        *)
            TESTS_FAILED=$((TESTS_FAILED + 1))
            printf '  FAIL %s\n       missing:  [%s]\n       in:       [%s]\n' "$3" "$2" "$1"
            ;;
    esac
}

# Build a real git repo. Git behaviour is what we depend on, so we never mock it.
make_repo() {
    _dir=$(mktemp -d "$TEST_TMP/repo.XXXXXX")
    git -C "$_dir" init -q
    git -C "$_dir" config user.name "Test"
    git -C "$_dir" config user.email "test@example.com"
    git -C "$_dir" checkout -q -b main
    echo "seed" > "$_dir/seed.txt"
    git -C "$_dir" add -A
    git -C "$_dir" commit -q -m "seed"
    printf '%s' "$_dir"
}

# stub_bin gh 'echo {}'  → a fake `gh` that prints {}
stub_bin() {
    printf '#!/bin/sh\n%s\n' "$2" > "$STUB_BIN/$1"
    chmod +x "$STUB_BIN/$1"
}

finish() {
    printf '\n  %d run, %d failed\n' "$TESTS_RUN" "$TESTS_FAILED"
    [ "$TESTS_FAILED" -eq 0 ] || exit 1
    exit 0
}
```

- [ ] **Step 2: Write the runner**

Create `tests/run.sh`:

```sh
#!/bin/sh
set -u
cd "$(dirname "$0")" || exit 1
overall=0
for t in test_*.sh; do
    [ -f "$t" ] || continue
    printf '\n%s\n' "$t"
    sh "$t" || overall=1
done
printf '\n'
[ "$overall" -eq 0 ] && printf 'ALL PASS\n' || printf 'FAILURES\n'
exit "$overall"
```

- [ ] **Step 3: Write the failing test for logging**

Create `tests/test_log.sh`:

```sh
#!/bin/sh
. "$(dirname "$0")/harness.sh"
. "$REPO_ROOT/lib/log.sh"

out=$(log_info "hello" 2>&1)
assert_contains "$out" "hello" "log_info emits the message"
assert_contains "$out" "INFO" "log_info is labelled"

out=$(log_error "boom" 2>&1)
assert_contains "$out" "ERROR" "log_error is labelled"

# Logging must never write to stdout — stdout carries return values between libs.
out=$(log_info "quiet" 2>/dev/null)
assert_eq "" "$out" "log_info writes nothing to stdout"

finish
```

- [ ] **Step 4: Run it and confirm it fails**

Run: `sh tests/test_log.sh`
Expected: fails because `lib/log.sh` does not exist — `No such file or directory`.

- [ ] **Step 5: Implement logging**

Create `lib/log.sh`:

```sh
#!/bin/sh
# Logging. Everything goes to stderr so that stdout stays clean for values.
# AUTOPILOT_LOG_FILE, when set, receives a copy.

_log() {
    _level=$1
    shift
    _line="$(date -u '+%Y-%m-%dT%H:%M:%SZ') $_level $*"
    printf '%s\n' "$_line" >&2
    if [ -n "${AUTOPILOT_LOG_FILE:-}" ]; then
        printf '%s\n' "$_line" >> "$AUTOPILOT_LOG_FILE"
    fi
}

log_info()  { _log INFO  "$@"; }
log_warn()  { _log WARN  "$@"; }
log_error() { _log ERROR "$@"; }
```

- [ ] **Step 6: Run it and confirm it passes**

Run: `sh tests/test_log.sh`
Expected: `4 run, 0 failed`

- [ ] **Step 7: Commit**

```bash
git add tests/harness.sh tests/run.sh tests/test_log.sh lib/log.sh
git commit -m "test: add shell test harness and logging

Harness builds real git repos rather than mocking git, because git's
behaviour is precisely what the runner depends on."
```

---

### Task 2: Config loading

**Files:**
- Create: `lib/config.sh`, `templates/config.json`, `tests/test_config.sh`

**Interfaces:**
- Consumes: `lib/log.sh`
- Produces: `cfg_load path` (loads and validates; returns 1 and logs on invalid), `cfg_get key default` (dotted jq path without the leading `.`, e.g. `agent.default_model`; prints the default when absent), `cfg_list key` (prints one line per array element).

- [ ] **Step 1: Write the template config**

Create `templates/config.json`:

```json
{
  "project": {
    "name": "CHANGE_ME",
    "main_branch": "main",
    "work_branch": "autopilot/main"
  },
  "queue": {
    "ready_label": "autopilot",
    "exclude_labels": ["blocked", "needs-human", "status:in-progress"],
    "depends_pattern": "Depends on #([0-9]+)"
  },
  "agent": {
    "permission_mode": "bypassPermissions",
    "default_model": "sonnet",
    "default_effort": "low",
    "turn_timeout_s": 2700,
    "max_budget_usd": 5.0
  },
  "verify": [
    {"name": "example", "cmd": "true"}
  ],
  "pacing": {
    "daily_task_cap": 12,
    "quiet_hours": [],
    "max_attempts_per_issue": 2,
    "circuit_breaker_failures": 3
  },
  "autonomy": {
    "default": "full",
    "prepare_only_labels": []
  }
}
```

- [ ] **Step 2: Write the failing test**

Create `tests/test_config.sh`:

```sh
#!/bin/sh
. "$(dirname "$0")/harness.sh"
. "$REPO_ROOT/lib/log.sh"
. "$REPO_ROOT/lib/config.sh"

good="$TEST_TMP/good.json"
cp "$REPO_ROOT/templates/config.json" "$good"

cfg_load "$good"
assert_eq "0" "$?" "cfg_load accepts the template"

assert_eq "sonnet" "$(cfg_get agent.default_model x)" "cfg_get reads a nested value"
assert_eq "fallback" "$(cfg_get agent.nonexistent fallback)" "cfg_get returns the default when absent"
assert_eq "main" "$(cfg_get project.main_branch x)" "cfg_get reads main_branch"

count=$(cfg_list queue.exclude_labels | wc -l | tr -d ' ')
assert_eq "3" "$count" "cfg_list emits one line per element"
assert_contains "$(cfg_list queue.exclude_labels)" "needs-human" "cfg_list content is correct"

# Malformed JSON must be rejected, not silently treated as empty.
bad="$TEST_TMP/bad.json"
printf '{ not json' > "$bad"
if cfg_load "$bad" 2>/dev/null; then rc=0; else rc=1; fi
assert_eq "1" "$rc" "cfg_load rejects malformed JSON"

# A missing required key must be rejected, so misconfiguration fails at the
# guard stage rather than halfway through a task.
missing="$TEST_TMP/missing.json"
jq 'del(.verify)' "$good" > "$missing"
if cfg_load "$missing" 2>/dev/null; then rc=0; else rc=1; fi
assert_eq "1" "$rc" "cfg_load rejects config with no verify block"

finish
```

- [ ] **Step 3: Run it and confirm it fails**

Run: `sh tests/test_config.sh`
Expected: fails — `lib/config.sh` does not exist.

- [ ] **Step 4: Implement config loading**

Create `lib/config.sh`:

```sh
#!/bin/sh
# Config loading. The config is the only place project-specific knowledge lives.

AUTOPILOT_CFG_FILE=""

# Keys without which the runner cannot safely proceed.
_CFG_REQUIRED="project.main_branch project.work_branch queue.ready_label verify agent.permission_mode"

cfg_load() {
    _file=$1
    if [ ! -f "$_file" ]; then
        log_error "config not found: $_file"
        return 1
    fi
    if ! jq -e . "$_file" >/dev/null 2>&1; then
        log_error "config is not valid JSON: $_file"
        return 1
    fi
    for _key in $_CFG_REQUIRED; do
        if ! jq -e ".$_key" "$_file" >/dev/null 2>&1; then
            log_error "config is missing required key: $_key"
            return 1
        fi
    done
    AUTOPILOT_CFG_FILE=$_file
    return 0
}

cfg_get() {
    _val=$(jq -r ".$1 // empty" "$AUTOPILOT_CFG_FILE" 2>/dev/null)
    if [ -z "$_val" ]; then
        printf '%s' "$2"
    else
        printf '%s' "$_val"
    fi
}

cfg_list() {
    jq -r ".$1[]? // empty" "$AUTOPILOT_CFG_FILE" 2>/dev/null
}
```

- [ ] **Step 5: Run it and confirm it passes**

Run: `sh tests/test_config.sh`
Expected: `8 run, 0 failed`

- [ ] **Step 6: Commit**

```bash
git add lib/config.sh templates/config.json tests/test_config.sh
git commit -m "feat: load and validate project config

Config is JSON rather than the YAML the spec sketched: yq is not installed
and parsing YAML in shell is not acceptable in code that runs with elevated
permissions. jq was already a required dependency."
```

---

### Task 3: State that round-trips

This task exists because the closest comparable system, Baton, has a `persist()` with no loader and stores its completed set as a count. For a process that exits after every task, restore is the entire point.

**Files:**
- Create: `lib/state.sh`, `tests/test_state.sh`

**Interfaces:**
- Consumes: `lib/log.sh`
- Produces: `state_init path` (creates the file with defaults if absent), `state_get key default`, `state_set key value` (string), `state_set_num key value` (number), `state_bump key` (increment a numeric field, print the new value).

State shape:

```json
{
  "resume_after": 0,
  "backoff_step": 0,
  "consecutive_failures": 0,
  "tasks_today": 0,
  "tasks_today_date": "",
  "last_issue": 0
}
```

- [ ] **Step 1: Write the failing test**

Create `tests/test_state.sh`:

```sh
#!/bin/sh
. "$(dirname "$0")/harness.sh"
. "$REPO_ROOT/lib/log.sh"
. "$REPO_ROOT/lib/state.sh"

f="$TEST_TMP/state.json"

state_init "$f"
assert_eq "0" "$(state_get resume_after -1)" "init writes defaults"

state_set_num resume_after 1755000000
assert_eq "1755000000" "$(state_get resume_after -1)" "numeric set then get"

state_set tasks_today_date "2026-08-14"
assert_eq "2026-08-14" "$(state_get tasks_today_date x)" "string set then get"

assert_eq "1" "$(state_bump tasks_today)" "bump returns the new value"
assert_eq "2" "$(state_bump tasks_today)" "bump accumulates"

# The invariant: a fresh process must see exactly what the last one wrote.
unset AUTOPILOT_STATE_FILE
state_init "$f"
assert_eq "1755000000" "$(state_get resume_after -1)" "round-trip: numbers survive re-init"
assert_eq "2026-08-14" "$(state_get tasks_today_date x)" "round-trip: strings survive re-init"
assert_eq "2" "$(state_get tasks_today -1)" "round-trip: counters survive re-init"

# init must not clobber an existing file.
state_init "$f"
assert_eq "2" "$(state_get tasks_today -1)" "init is idempotent, never resets"

# A corrupted state file must be replaced, not crash the run.
printf 'garbage' > "$f"
state_init "$f"
assert_eq "0" "$(state_get tasks_today -1)" "corrupt state is rebuilt from defaults"

finish
```

- [ ] **Step 2: Run it and confirm it fails**

Run: `sh tests/test_state.sh`
Expected: fails — `lib/state.sh` does not exist.

- [ ] **Step 3: Implement state**

Create `lib/state.sh`:

```sh
#!/bin/sh
# Run state. This file is the runner's only memory between invocations,
# so every write must be readable back into the same shape.

AUTOPILOT_STATE_FILE=""

_STATE_DEFAULTS='{
  "resume_after": 0,
  "backoff_step": 0,
  "consecutive_failures": 0,
  "tasks_today": 0,
  "tasks_today_date": "",
  "last_issue": 0
}'

state_init() {
    AUTOPILOT_STATE_FILE=$1
    if [ ! -f "$AUTOPILOT_STATE_FILE" ] || ! jq -e . "$AUTOPILOT_STATE_FILE" >/dev/null 2>&1; then
        [ -f "$AUTOPILOT_STATE_FILE" ] && log_warn "state file unreadable, rebuilding from defaults"
        mkdir -p "$(dirname "$AUTOPILOT_STATE_FILE")"
        printf '%s\n' "$_STATE_DEFAULTS" > "$AUTOPILOT_STATE_FILE"
    fi
    return 0
}

state_get() {
    _val=$(jq -r ".$1 // empty" "$AUTOPILOT_STATE_FILE" 2>/dev/null)
    if [ -z "$_val" ]; then printf '%s' "$2"; else printf '%s' "$_val"; fi
}

# Writes go through a temp file so an interrupted run cannot leave a truncated
# state file behind — that would look identical to corruption on the next wake.
_state_write() {
    _tmp="$AUTOPILOT_STATE_FILE.tmp.$$"
    if jq "$1" "$AUTOPILOT_STATE_FILE" > "$_tmp" 2>/dev/null; then
        mv "$_tmp" "$AUTOPILOT_STATE_FILE"
    else
        rm -f "$_tmp"
        log_error "state write failed: $1"
        return 1
    fi
}

state_set()     { _state_write ".$1 = \"$2\""; }
state_set_num() { _state_write ".$1 = $2"; }

state_bump() {
    _new=$(( $(state_get "$1" 0) + 1 ))
    _state_write ".$1 = $_new" && printf '%s' "$_new"
}
```

- [ ] **Step 4: Run it and confirm it passes**

Run: `sh tests/test_state.sh`
Expected: `11 run, 0 failed`

- [ ] **Step 5: Commit**

```bash
git add lib/state.sh tests/test_state.sh
git commit -m "feat: add state that round-trips across invocations

Writes go via a temp file and rename: an interrupted run must not leave a
truncated state file, which the next wake cannot distinguish from corruption.
Tested by re-initialising and reading back every field type."
```

---

### Task 4: Guards

**Files:**
- Create: `lib/guards.sh`, `tests/test_guards.sh`

**Interfaces:**
- Consumes: `lib/log.sh`, `lib/config.sh`, `lib/state.sh`
- Produces: `guard_all project_root` — returns 0 to proceed, 1 to stop quietly. Individual guards, each returning 0 to proceed: `guard_stop_file dir`, `guard_lock dir` (acquires; `guard_unlock dir` releases), `guard_resume_after`, `guard_daily_cap`, `guard_quiet_hours`, `guard_project_present root`.

- [ ] **Step 1: Write the failing test**

Create `tests/test_guards.sh`:

```sh
#!/bin/sh
. "$(dirname "$0")/harness.sh"
. "$REPO_ROOT/lib/log.sh"
. "$REPO_ROOT/lib/config.sh"
. "$REPO_ROOT/lib/state.sh"
. "$REPO_ROOT/lib/guards.sh"

repo=$(make_repo)
mkdir -p "$repo/.autopilot"
cp "$REPO_ROOT/templates/config.json" "$repo/.autopilot/config.json"
cfg_load "$repo/.autopilot/config.json"
state_init "$repo/.autopilot/state.json"

# --- STOP file ---
if guard_stop_file "$repo/.autopilot"; then rc=0; else rc=1; fi
assert_eq "0" "$rc" "no STOP file means proceed"
touch "$repo/.autopilot/STOP"
if guard_stop_file "$repo/.autopilot" 2>/dev/null; then rc=0; else rc=1; fi
assert_eq "1" "$rc" "STOP file halts the run"
rm "$repo/.autopilot/STOP"

# --- lock ---
if guard_lock "$repo/.autopilot"; then rc=0; else rc=1; fi
assert_eq "0" "$rc" "first lock acquisition succeeds"
if guard_lock "$repo/.autopilot" 2>/dev/null; then rc=0; else rc=1; fi
assert_eq "1" "$rc" "second acquisition is refused while held"
guard_unlock "$repo/.autopilot"
if guard_lock "$repo/.autopilot"; then rc=0; else rc=1; fi
assert_eq "0" "$rc" "lock is reacquirable after release"
guard_unlock "$repo/.autopilot"

# A lock left by a dead process must not wedge the loop forever.
mkdir -p "$repo/.autopilot/lock"
printf '999999' > "$repo/.autopilot/lock/pid"
if guard_lock "$repo/.autopilot" 2>/dev/null; then rc=0; else rc=1; fi
assert_eq "0" "$rc" "stale lock from a dead pid is broken"
guard_unlock "$repo/.autopilot"

# --- resume_after ---
state_set_num resume_after 0
if guard_resume_after; then rc=0; else rc=1; fi
assert_eq "0" "$rc" "zero resume_after means proceed"
state_set_num resume_after "$(( $(date +%s) + 3600 ))"
if guard_resume_after 2>/dev/null; then rc=0; else rc=1; fi
assert_eq "1" "$rc" "future resume_after halts the run"
state_set_num resume_after "$(( $(date +%s) - 60 ))"
if guard_resume_after; then rc=0; else rc=1; fi
assert_eq "0" "$rc" "past resume_after means proceed"

# --- daily cap ---
state_set tasks_today_date "$(date -u +%Y-%m-%d)"
state_set_num tasks_today 11
if guard_daily_cap; then rc=0; else rc=1; fi
assert_eq "0" "$rc" "below the cap means proceed"
state_set_num tasks_today 12
if guard_daily_cap 2>/dev/null; then rc=0; else rc=1; fi
assert_eq "1" "$rc" "at the cap halts the run"

# The counter must reset when the date rolls over, or the loop dies after one day.
state_set tasks_today_date "2020-01-01"
if guard_daily_cap; then rc=0; else rc=1; fi
assert_eq "0" "$rc" "a new day resets the counter"
assert_eq "0" "$(state_get tasks_today -1)" "counter is actually zeroed on rollover"

# --- project present ---
if guard_project_present "$repo"; then rc=0; else rc=1; fi
assert_eq "0" "$rc" "existing project root means proceed"
if guard_project_present "/nonexistent/volume/project" 2>/dev/null; then rc=0; else rc=1; fi
assert_eq "1" "$rc" "missing project root halts the run"

finish
```

- [ ] **Step 2: Run it and confirm it fails**

Run: `sh tests/test_guards.sh`
Expected: fails — `lib/guards.sh` does not exist.

- [ ] **Step 3: Implement guards**

Create `lib/guards.sh`:

```sh
#!/bin/sh
# Guards. Each answers one question: should this run proceed?
# Returning 1 is a normal, quiet outcome — not an error.

guard_stop_file() {
    if [ -e "$1/STOP" ]; then
        log_info "STOP file present, standing down"
        return 1
    fi
    return 0
}

# mkdir is atomic on every filesystem we care about, so it is the lock.
guard_lock() {
    _lock="$1/lock"
    if mkdir "$_lock" 2>/dev/null; then
        printf '%s' "$$" > "$_lock/pid"
        return 0
    fi
    _pid=$(cat "$_lock/pid" 2>/dev/null || printf '0')
    if [ "$_pid" != "0" ] && kill -0 "$_pid" 2>/dev/null; then
        log_info "another run is in progress (pid $_pid)"
        return 1
    fi
    log_warn "breaking stale lock from pid $_pid"
    rm -rf "$_lock"
    if mkdir "$_lock" 2>/dev/null; then
        printf '%s' "$$" > "$_lock/pid"
        return 0
    fi
    log_info "could not acquire lock"
    return 1
}

guard_unlock() { rm -rf "$1/lock"; }

guard_resume_after() {
    _until=$(state_get resume_after 0)
    _now=$(date +%s)
    if [ "$_until" -gt "$_now" ]; then
        log_info "usage window closed until $(date -r "$_until" '+%H:%M' 2>/dev/null || printf '%s' "$_until")"
        return 1
    fi
    return 0
}

guard_daily_cap() {
    _today=$(date -u +%Y-%m-%d)
    if [ "$(state_get tasks_today_date '')" != "$_today" ]; then
        state_set tasks_today_date "$_today"
        state_set_num tasks_today 0
    fi
    _cap=$(cfg_get pacing.daily_task_cap 12)
    _done=$(state_get tasks_today 0)
    if [ "$_done" -ge "$_cap" ]; then
        log_info "daily cap reached ($_done/$_cap)"
        return 1
    fi
    return 0
}

# quiet_hours entries are "HH:MM-HH:MM" local. Inside one means stand down.
guard_quiet_hours() {
    _now=$(date +%H%M | sed 's/^0*//')
    [ -z "$_now" ] && _now=0
    for _range in $(cfg_list pacing.quiet_hours); do
        _from=$(printf '%s' "$_range" | cut -d- -f1 | tr -d ':' | sed 's/^0*//')
        _to=$(printf '%s' "$_range" | cut -d- -f2 | tr -d ':' | sed 's/^0*//')
        [ -z "$_from" ] && _from=0
        [ -z "$_to" ] && _to=0
        if [ "$_from" -le "$_to" ]; then
            if [ "$_now" -ge "$_from" ] && [ "$_now" -lt "$_to" ]; then
                log_info "inside quiet hours $_range"
                return 1
            fi
        else
            # Range wraps midnight.
            if [ "$_now" -ge "$_from" ] || [ "$_now" -lt "$_to" ]; then
                log_info "inside quiet hours $_range"
                return 1
            fi
        fi
    done
    return 0
}

# The project may live on a volume that is not mounted.
guard_project_present() {
    if [ ! -d "$1/.git" ]; then
        log_info "project root unavailable: $1"
        return 1
    fi
    return 0
}

guard_all() {
    _root=$1
    _dir="$_root/.autopilot"
    guard_project_present "$_root" || return 1
    guard_stop_file "$_dir"        || return 1
    guard_resume_after             || return 1
    guard_quiet_hours              || return 1
    guard_daily_cap                || return 1
    guard_lock "$_dir"             || return 1
    return 0
}
```

- [ ] **Step 4: Run it and confirm it passes**

Run: `sh tests/test_guards.sh`
Expected: `16 run, 0 failed`

- [ ] **Step 5: Commit**

```bash
git add lib/guards.sh tests/test_guards.sh
git commit -m "feat: add run guards

A lock held by a dead process is broken rather than respected: a crash during
a run would otherwise wedge the loop permanently, and the loop has no operator
watching it. The daily counter resets on date rollover for the same reason."
```

---

### Task 5: Queue

**Files:**
- Create: `lib/queue.sh`, `tests/test_queue.sh`, `tests/fixtures/gh-issue-list.json`

**Interfaces:**
- Consumes: `lib/log.sh`, `lib/config.sh`
- Produces: `queue_candidates` (prints the raw `gh issue list --json` payload), `queue_deps body` (prints each dependency number on its own line), `queue_is_closed number` (0 if closed or absent), `queue_pick` (prints the number of the first eligible issue, empty if none), `queue_field number key` (prints one field of a cached issue), `queue_claim number`, `queue_release number`.

- [ ] **Step 1: Record the fixture**

Run against the real repository so the shape is real, never invented:

```bash
gh issue list --repo tmthang86/the companion project --state open --limit 5 \
  --json number,title,body,labels,milestone > tests/fixtures/gh-issue-list.json
```

If no issues exist yet, create one throwaway issue first, record, then close it. **Do not hand-write this file.**

- [ ] **Step 2: Write the failing test**

Create `tests/test_queue.sh`:

```sh
#!/bin/sh
. "$(dirname "$0")/harness.sh"
. "$REPO_ROOT/lib/log.sh"
. "$REPO_ROOT/lib/config.sh"
. "$REPO_ROOT/lib/queue.sh"

repo=$(make_repo)
mkdir -p "$repo/.autopilot"
cp "$REPO_ROOT/templates/config.json" "$repo/.autopilot/config.json"
cfg_load "$repo/.autopilot/config.json"

# Dependency parsing is pure text work and gets tested on its own.
assert_eq "7" "$(queue_deps 'Blah blah. Depends on #7. More text.')" "single dependency parsed"
two=$(queue_deps 'Depends on #7
Depends on #12')
assert_contains "$two" "7"  "first of two dependencies parsed"
assert_contains "$two" "12" "second of two dependencies parsed"
assert_eq "" "$(queue_deps 'No dependencies here at all')" "no dependency yields empty"
assert_eq "" "$(queue_deps 'See issue #7 for context')" "a bare issue reference is not a dependency"

# Selection, with gh stubbed. #10 depends on #11, which is still open,
# so #11 must be chosen even though #10 is listed first.
stub_bin gh 'case "$*" in
  *"issue list"*) cat <<JSON
[{"number":10,"title":"Blocked one","body":"Depends on #11","labels":[{"name":"autopilot"}],"milestone":null},
 {"number":11,"title":"Ready one","body":"No deps","labels":[{"name":"autopilot"}],"milestone":null}]
JSON
  ;;
  *"issue view"*11*) echo "{\"state\":\"OPEN\"}" ;;
  *"issue view"*) echo "{\"state\":\"OPEN\"}" ;;
  *) echo "{}" ;;
esac'

assert_eq "11" "$(queue_pick)" "a blocked issue is skipped in favour of its dependency"

# Once the dependency is closed, the blocked issue becomes eligible.
stub_bin gh 'case "$*" in
  *"issue list"*) cat <<JSON
[{"number":10,"title":"Now ready","body":"Depends on #11","labels":[{"name":"autopilot"}],"milestone":null}]
JSON
  ;;
  *"issue view"*) echo "{\"state\":\"CLOSED\"}" ;;
  *) echo "{}" ;;
esac'
assert_eq "10" "$(queue_pick)" "issue becomes eligible once its dependency closes"

# An empty queue is a normal outcome, not an error.
stub_bin gh 'case "$*" in *"issue list"*) echo "[]" ;; *) echo "{}" ;; esac'
assert_eq "" "$(queue_pick)" "empty queue yields empty, not an error"

finish
```

- [ ] **Step 3: Run it and confirm it fails**

Run: `sh tests/test_queue.sh`
Expected: fails — `lib/queue.sh` does not exist.

- [ ] **Step 4: Implement the queue**

Create `lib/queue.sh`:

```sh
#!/bin/sh
# The queue. This is the only file that knows GitHub exists.

QUEUE_CACHE=""

queue_candidates() {
    _args="issue list --state open --limit 50 --json number,title,body,labels,milestone"
    _args="$_args --label $(cfg_get queue.ready_label autopilot)"
    # shellcheck disable=SC2086
    gh $_args 2>/dev/null || printf '[]'
}

# Only the configured phrase counts. A bare "#7" is a reference, not a dependency.
queue_deps() {
    printf '%s\n' "$1" | sed -n 's/.*[Dd]epends on #\([0-9][0-9]*\).*/\1/p'
}

queue_is_closed() {
    _state=$(gh issue view "$1" --json state -q .state 2>/dev/null || printf 'CLOSED')
    [ "$_state" != "OPEN" ]
}

queue_pick() {
    QUEUE_CACHE=$(queue_candidates)
    _excluded=$(cfg_list queue.exclude_labels | tr '\n' ' ')
    _count=$(printf '%s' "$QUEUE_CACHE" | jq 'length')
    _i=0
    while [ "$_i" -lt "$_count" ]; do
        _num=$(printf '%s' "$QUEUE_CACHE"  | jq -r ".[$_i].number")
        _body=$(printf '%s' "$QUEUE_CACHE" | jq -r ".[$_i].body // \"\"")
        _labels=$(printf '%s' "$QUEUE_CACHE" | jq -r ".[$_i].labels[].name" 2>/dev/null)
        _i=$((_i + 1))

        _skip=0
        for _ex in $_excluded; do
            for _lb in $_labels; do
                [ "$_lb" = "$_ex" ] && _skip=1
            done
        done
        [ "$_skip" -eq 1 ] && continue

        _blocked=0
        for _dep in $(queue_deps "$_body"); do
            queue_is_closed "$_dep" || _blocked=1
        done
        [ "$_blocked" -eq 1 ] && continue

        printf '%s' "$_num"
        return 0
    done
    return 0
}

queue_field() {
    printf '%s' "$QUEUE_CACHE" | jq -r ".[] | select(.number == $1) | .$2 // \"\""
}

queue_claim()   { gh issue edit "$1" --add-label "status:in-progress" >/dev/null 2>&1; }
queue_release() { gh issue edit "$1" --remove-label "status:in-progress" >/dev/null 2>&1; }
```

- [ ] **Step 5: Run it and confirm it passes**

Run: `sh tests/test_queue.sh`
Expected: `8 run, 0 failed`

- [ ] **Step 6: Commit**

```bash
git add lib/queue.sh tests/test_queue.sh tests/fixtures/gh-issue-list.json
git commit -m "feat: add queue with dependency-aware selection

GitHub Issues has no notion of ready work, so dependencies are declared in the
body and resolved here. A bare '#7' is deliberately not a dependency — only the
configured phrase counts, so ordinary cross-references stay harmless."
```

---

### Task 6: Agent invocation and outcome classification

This carries invariant 2. Getting it wrong means a closed usage window consumes an innocent task's retry budget.

**Files:**
- Create: `lib/agent.sh`, `tests/test_agent.sh`, `tests/fixtures/claude-result-success.json`
- Create: `templates/prompt.tmpl`

**Interfaces:**
- Consumes: `lib/log.sh`, `lib/config.sh`
- Produces: `agent_args model effort` (prints the argument list), `agent_run prompt cwd logfile` (runs it; returns 0 on success, 1 on failure), `agent_classify logfile exit_code` (prints exactly one of `ok`, `usage_limit`, `task_failure`), `agent_probe` (returns 0 if the account can still serve requests).

- [ ] **Step 1: Record the fixture**

```bash
claude -p "say OK" --output-format json --tools "" --model sonnet \
  --no-session-persistence > tests/fixtures/claude-result-success.json
```

- [ ] **Step 2: Write the prompt template**

Create `templates/prompt.tmpl`. The leading block is byte-identical on every run so the provider's prompt cache absorbs it; only the task block below changes.

```
You are running unattended as part of an automated delivery loop.

Rules that override any instinct to be helpful:
- Implement only what the task below describes. It comes from a plan a human approved.
- If you meet a decision the task does not cover, do NOT choose. Post the question as a
  comment on the issue, add the label "blocked", and stop. Moving on is correct behaviour.
- Do not create plans, do not modify CLAUDE.md, do not alter an accepted decision record.
- Never force-push. Never check out the project's main branch.
- Do not claim anything passes without running the command and reading its output.
- Every commit body must say WHY. The diff already says what.

You are on branch {{WORK_BRANCH}} of {{PROJECT_NAME}}.
Verification is run by the harness after you finish; a red suite discards your work,
so run the project's checks yourself before you declare completion.

--- TASK #{{ISSUE_NUMBER}}: {{ISSUE_TITLE}} ---

{{ISSUE_BODY}}
```

- [ ] **Step 3: Write the failing test**

Create `tests/test_agent.sh`:

```sh
#!/bin/sh
. "$(dirname "$0")/harness.sh"
. "$REPO_ROOT/lib/log.sh"
. "$REPO_ROOT/lib/config.sh"
. "$REPO_ROOT/lib/agent.sh"

repo=$(make_repo)
mkdir -p "$repo/.autopilot"
cp "$REPO_ROOT/templates/config.json" "$repo/.autopilot/config.json"
cfg_load "$repo/.autopilot/config.json"

args=$(agent_args sonnet low)
assert_contains "$args" "--model sonnet"                  "model is passed"
assert_contains "$args" "--effort low"                    "effort is passed"
assert_contains "$args" "--max-budget-usd"                "spend ceiling is passed"
assert_contains "$args" "--dangerously-skip-permissions"  "bypassPermissions maps to the real flag"
assert_contains "$args" "--add-dir"                       "directory scope is passed"

log="$TEST_TMP/run.log"

# Success.
printf '{"type":"result","is_error":false,"result":"done"}\n' > "$log"
assert_eq "ok" "$(agent_classify "$log" 0)" "clean result classifies as ok"

# An explicit rate-limit event in the stream is decisive on its own.
printf '{"type":"rate_limit_event","rate_limit_info":{"status":"rejected"}}\n' > "$log"
assert_eq "usage_limit" "$(agent_classify "$log" 1)" "rate_limit_event classifies as usage_limit"

# Ambiguous failure, probe succeeds → the task is at fault.
printf '{"type":"result","is_error":true,"result":"compile error"}\n' > "$log"
stub_bin claude 'echo "{\"is_error\":false,\"result\":\"ok\"}"; exit 0'
assert_eq "task_failure" "$(agent_classify "$log" 1)" "probe succeeds, so the failure belongs to the task"

# Ambiguous failure, probe also fails → the account is throttled, task is innocent.
stub_bin claude 'exit 1'
assert_eq "usage_limit" "$(agent_classify "$log" 1)" "probe fails, so the account is throttled"

# A vanished log must not be read as success.
rm -f "$log"
stub_bin claude 'echo "{\"is_error\":false}"; exit 0'
assert_eq "task_failure" "$(agent_classify "$log" 1)" "missing log with nonzero exit is a task failure"

finish
```

- [ ] **Step 4: Run it and confirm it fails**

Run: `sh tests/test_agent.sh`
Expected: fails — `lib/agent.sh` does not exist.

- [ ] **Step 5: Implement the agent layer**

Create `lib/agent.sh`:

```sh
#!/bin/sh
# Agent invocation and, more importantly, classification of what came back.

agent_args() {
    _mode=$(cfg_get agent.permission_mode bypassPermissions)
    case "$_mode" in
        bypassPermissions) _perm="--dangerously-skip-permissions" ;;
        *)                 _perm="--permission-mode $_mode" ;;
    esac
    printf -- '-p --output-format stream-json --verbose %s --model %s --effort %s --max-budget-usd %s --add-dir . ' \
        "$_perm" "$1" "$2" "$(cfg_get agent.max_budget_usd 5.0)"
}

agent_run() {
    _prompt=$1; _cwd=$2; _log=$3
    # shellcheck disable=SC2086
    ( cd "$_cwd" && claude $(agent_args "${AGENT_MODEL:-sonnet}" "${AGENT_EFFORT:-low}") "$_prompt" ) > "$_log" 2>&1
}

# One trivial call. If it fails too, the account is throttled and the task is
# innocent. This is deliberately empirical: the throttled payload shape is not
# documented, so we do not parse it.
agent_probe() {
    claude -p "ok" --model sonnet --tools "" --no-session-persistence >/dev/null 2>&1
}

agent_classify() {
    _log=$1; _rc=$2

    if [ ! -f "$_log" ]; then
        [ "$_rc" -eq 0 ] && printf 'ok' || printf 'task_failure'
        return 0
    fi

    if grep -q '"rate_limit_info"' "$_log" 2>/dev/null &&
       ! grep -q '"status":"allowed"' "$_log" 2>/dev/null; then
        printf 'usage_limit'
        return 0
    fi

    if [ "$_rc" -eq 0 ] && ! grep -q '"is_error":true' "$_log" 2>/dev/null; then
        printf 'ok'
        return 0
    fi

    if agent_probe; then
        printf 'task_failure'
    else
        log_warn "probe failed — attributing to usage limit, not to the task"
        printf 'usage_limit'
    fi
}
```

- [ ] **Step 6: Run it and confirm it passes**

Run: `sh tests/test_agent.sh`
Expected: `11 run, 0 failed`

- [ ] **Step 7: Commit**

```bash
git add lib/agent.sh templates/prompt.tmpl tests/test_agent.sh tests/fixtures/claude-result-success.json
git commit -m "feat: classify agent outcomes before choosing a response

The reason this is its own concern: a closed usage window and a compile error
arrive through the same channel. Baton conflates them and answers both with
exponential backoff, so an outage silently consumes an innocent task's retry
budget. When the stream is inconclusive we probe with one trivial call rather
than parsing an undocumented payload shape."
```

---

### Task 7: Verification gate

This carries invariant 1.

**Files:**
- Create: `lib/verify.sh`, `tests/test_verify.sh`

**Interfaces:**
- Consumes: `lib/log.sh`, `lib/config.sh`
- Produces: `verify_run cwd` — runs every configured command in order, stops at the first failure, returns 0 only if all passed. `VERIFY_FAILED_NAME` and `VERIFY_OUTPUT` are set for the caller.

- [ ] **Step 1: Write the failing test**

Create `tests/test_verify.sh`:

```sh
#!/bin/sh
. "$(dirname "$0")/harness.sh"
. "$REPO_ROOT/lib/log.sh"
. "$REPO_ROOT/lib/config.sh"
. "$REPO_ROOT/lib/verify.sh"

repo=$(make_repo)
mkdir -p "$repo/.autopilot"

mkcfg() { jq ".verify = $1" "$REPO_ROOT/templates/config.json" > "$repo/.autopilot/config.json"; cfg_load "$repo/.autopilot/config.json"; }

mkcfg '[{"name":"a","cmd":"true"},{"name":"b","cmd":"true"}]'
if verify_run "$repo"; then rc=0; else rc=1; fi
assert_eq "0" "$rc" "all commands passing returns success"

mkcfg '[{"name":"a","cmd":"true"},{"name":"b","cmd":"echo boom >&2; false"},{"name":"c","cmd":"true"}]'
if verify_run "$repo" 2>/dev/null; then rc=0; else rc=1; fi
assert_eq "1" "$rc" "one failing command fails the whole gate"
assert_eq "b" "$VERIFY_FAILED_NAME" "the failing command is named"
assert_contains "$VERIFY_OUTPUT" "boom" "the failure output is captured"

# An empty verify list must NOT pass. A project with no checks has no oracle,
# and silently allowing commits would defeat the entire gate.
mkcfg '[]'
if verify_run "$repo" 2>/dev/null; then rc=0; else rc=1; fi
assert_eq "1" "$rc" "an empty verify list is refused, not treated as success"

# Commands must run in the project, not wherever the runner happens to be.
mkcfg '[{"name":"cwd","cmd":"test -f seed.txt"}]'
if verify_run "$repo"; then rc=0; else rc=1; fi
assert_eq "0" "$rc" "commands run inside the project root"

finish
```

- [ ] **Step 2: Run it and confirm it fails**

Run: `sh tests/test_verify.sh`
Expected: fails — `lib/verify.sh` does not exist.

- [ ] **Step 3: Implement verification**

Create `lib/verify.sh`:

```sh
#!/bin/sh
# The verification gate. Nothing gets committed unless this returns 0.

VERIFY_FAILED_NAME=""
VERIFY_OUTPUT=""

verify_run() {
    _cwd=$1
    VERIFY_FAILED_NAME=""
    VERIFY_OUTPUT=""

    _count=$(jq '.verify | length' "$AUTOPILOT_CFG_FILE" 2>/dev/null || printf '0')
    if [ "$_count" -eq 0 ]; then
        log_error "no verify commands configured — refusing to treat unverified work as done"
        return 1
    fi

    _i=0
    while [ "$_i" -lt "$_count" ]; do
        _name=$(jq -r ".verify[$_i].name" "$AUTOPILOT_CFG_FILE")
        _cmd=$(jq -r ".verify[$_i].cmd" "$AUTOPILOT_CFG_FILE")
        _i=$((_i + 1))

        log_info "verify: $_name"
        _out=$( (cd "$_cwd" && sh -c "$_cmd") 2>&1 ) || {
            VERIFY_FAILED_NAME=$_name
            VERIFY_OUTPUT=$_out
            log_error "verify failed at: $_name"
            return 1
        }
    done
    return 0
}
```

- [ ] **Step 4: Run it and confirm it passes**

Run: `sh tests/test_verify.sh`
Expected: `7 run, 0 failed`

- [ ] **Step 5: Commit**

```bash
git add lib/verify.sh tests/test_verify.sh
git commit -m "feat: add the verification gate

An empty verify list fails rather than passes. A project with no checks has no
oracle, and treating that as success would let the runner commit unverified work
while appearing to honour the gate."
```

---

### Task 8: Settlement

**Files:**
- Create: `lib/settle.sh`, `tests/test_settle.sh`

**Interfaces:**
- Consumes: `lib/log.sh`, `lib/config.sh`, `lib/queue.sh`
- Produces: `settle_success root number title prepare_only`, `settle_failure root number reason detail`, `settle_blocked root number question`.

- [ ] **Step 1: Write the failing test**

Create `tests/test_settle.sh`:

```sh
#!/bin/sh
. "$(dirname "$0")/harness.sh"
. "$REPO_ROOT/lib/log.sh"
. "$REPO_ROOT/lib/config.sh"
. "$REPO_ROOT/lib/queue.sh"
. "$REPO_ROOT/lib/settle.sh"

repo=$(make_repo)
mkdir -p "$repo/.autopilot"
cp "$REPO_ROOT/templates/config.json" "$repo/.autopilot/config.json"
cfg_load "$repo/.autopilot/config.json"

stub_bin gh 'echo "$*" >> "$GH_CALLS"; exit 0'
GH_CALLS="$TEST_TMP/gh-calls.txt"; export GH_CALLS
: > "$GH_CALLS"

git -C "$repo" checkout -q -b autopilot/main
echo "work" > "$repo/new.txt"

settle_success "$repo" 42 "Add a thing" 0
msg=$(git -C "$repo" log -1 --pretty=%B)
assert_contains "$msg" "Closes #42" "commit carries the closing trailer"
assert_eq "" "$(git -C "$repo" status --porcelain)" "working tree is clean after settling"
assert_contains "$(cat "$GH_CALLS")" "issue close 42" "issue is closed on full autonomy"

# prepare_only work must be committed but NOT closed — this is the ceiling
# that keeps the Definition of Done honest for anything a human must see.
: > "$GH_CALLS"
echo "more" > "$repo/other.txt"
settle_success "$repo" 43 "UI work" 1
calls=$(cat "$GH_CALLS")
assert_contains "$calls" "needs-human"        "prepare-only work is labelled for a human"
case "$calls" in *"issue close 43"*) closed=1 ;; *) closed=0 ;; esac
assert_eq "0" "$closed" "prepare-only work is NOT closed by the runner"

# A failed run must leave nothing behind.
echo "junk" > "$repo/junk.txt"
: > "$GH_CALLS"
settle_failure "$repo" 44 "verify" "clippy said no"
assert_eq "" "$(git -C "$repo" status --porcelain)" "failure resets the working tree"
assert_contains "$(cat "$GH_CALLS")" "issue comment 44" "failure is reported on the issue"

finish
```

- [ ] **Step 2: Run it and confirm it fails**

Run: `sh tests/test_settle.sh`
Expected: fails — `lib/settle.sh` does not exist.

- [ ] **Step 3: Implement settlement**

Create `lib/settle.sh`:

```sh
#!/bin/sh
# Settlement. The only file that writes to git history.

_settle_guard_branch() {
    _cur=$(git -C "$1" rev-parse --abbrev-ref HEAD)
    _main=$(cfg_get project.main_branch main)
    if [ "$_cur" = "$_main" ]; then
        log_error "refusing to settle on the main branch"
        return 1
    fi
    return 0
}

settle_success() {
    _root=$1; _num=$2; _title=$3; _prepare_only=$4
    _settle_guard_branch "$_root" || return 1

    git -C "$_root" add -A
    if git -C "$_root" diff --cached --quiet; then
        log_warn "nothing to commit for #$_num"
    else
        git -C "$_root" commit -q -m "$_title

Implements the task described in issue #$_num, which comes from the
approved plan. Verification passed before this commit was made.

Closes #$_num"
        git -C "$_root" push -q origin HEAD 2>/dev/null || log_warn "push failed; commit is local"
    fi

    queue_release "$_num"
    if [ "$_prepare_only" -eq 1 ]; then
        gh issue edit "$_num" --add-label "needs-human" >/dev/null 2>&1
        gh issue comment "$_num" --body "Implementation complete and verified by the automated checks. This task's correctness depends on behaviour a person has to observe, so it stays open for human acceptance." >/dev/null 2>&1
        log_info "#$_num left open for human acceptance"
    else
        gh issue comment "$_num" --body "Completed automatically. Verification passed and the work is committed." >/dev/null 2>&1
        gh issue close "$_num" >/dev/null 2>&1
        log_info "#$_num closed"
    fi
}

settle_failure() {
    _root=$1; _num=$2; _reason=$3; _detail=$4
    git -C "$_root" reset -q --hard
    git -C "$_root" clean -qfd
    gh issue comment "$_num" --body "Automated attempt failed at: $_reason

\`\`\`
$_detail
\`\`\`

The working tree was reset; nothing was committed." >/dev/null 2>&1
    queue_release "$_num"
    log_info "#$_num released after failure at $_reason"
}

settle_blocked() {
    _root=$1; _num=$2; _question=$3
    git -C "$_root" reset -q --hard
    git -C "$_root" clean -qfd
    gh issue comment "$_num" --body "Blocked — this needs a decision that the approved plan does not cover:

$_question" >/dev/null 2>&1
    gh issue edit "$_num" --add-label "blocked" >/dev/null 2>&1
    queue_release "$_num"
    log_info "#$_num blocked pending a decision"
}
```

- [ ] **Step 4: Run it and confirm it passes**

Run: `sh tests/test_settle.sh`
Expected: `7 run, 0 failed`

- [ ] **Step 5: Commit**

```bash
git add lib/settle.sh tests/test_settle.sh
git commit -m "feat: add settlement paths for success, failure, and blocked

prepare_only work is committed but never closed. If the runner could close work
whose correctness lives on a screen, the Definition of Done would become a
formality — so the ceiling is enforced in code, not only in prose."
```

---

### Task 9: Entry point

**Files:**
- Create: `run-once.sh`, `tests/test_run_once.sh`

**Interfaces:**
- Consumes: every lib
- Produces: the executable entry point. `run-once.sh --project <path>`. Exit 0 for "ran, or correctly declined to run"; exit 1 only for a configuration error.

- [ ] **Step 1: Write the failing test**

Create `tests/test_run_once.sh`:

```sh
#!/bin/sh
. "$(dirname "$0")/harness.sh"

repo=$(make_repo)
mkdir -p "$repo/.autopilot"
jq '.verify = [{"name":"t","cmd":"true"}]' "$REPO_ROOT/templates/config.json" > "$repo/.autopilot/config.json"

# A STOP file must short-circuit before anything else happens.
touch "$repo/.autopilot/STOP"
stub_bin gh 'echo "SHOULD NOT BE CALLED" >> "$GH_CALLS"; exit 0'
GH_CALLS="$TEST_TMP/calls.txt"; export GH_CALLS
: > "$GH_CALLS"
sh "$REPO_ROOT/run-once.sh" --project "$repo" >/dev/null 2>&1
assert_eq "0" "$?" "STOP file yields a clean exit"
assert_eq "" "$(cat "$GH_CALLS")" "STOP file prevents any queue call"
rm "$repo/.autopilot/STOP"

# An empty queue is a normal no-op and must release the lock on the way out.
stub_bin gh 'case "$*" in *"issue list"*) echo "[]" ;; *) echo "{}" ;; esac'
sh "$REPO_ROOT/run-once.sh" --project "$repo" >/dev/null 2>&1
assert_eq "0" "$?" "empty queue yields a clean exit"
assert_eq "0" "$([ -d "$repo/.autopilot/lock" ] && echo 1 || echo 0)" "lock is released on the empty-queue path"

# A missing config is an operator error and must be loud.
bare=$(make_repo)
sh "$REPO_ROOT/run-once.sh" --project "$bare" >/dev/null 2>&1
assert_eq "1" "$?" "missing config exits non-zero"

finish
```

- [ ] **Step 2: Run it and confirm it fails**

Run: `sh tests/test_run_once.sh`
Expected: fails — `run-once.sh` does not exist.

- [ ] **Step 3: Implement the entry point**

Create `run-once.sh`:

```sh
#!/bin/sh
# Autopilot: one task per invocation, then exit.
set -eu

HERE=$(cd "$(dirname "$0")" && pwd)
PROJECT=""

while [ $# -gt 0 ]; do
    case "$1" in
        --project) PROJECT=$2; shift 2 ;;
        *) printf 'usage: run-once.sh --project <path>\n' >&2; exit 1 ;;
    esac
done
[ -n "$PROJECT" ] || { printf 'usage: run-once.sh --project <path>\n' >&2; exit 1; }

. "$HERE/lib/log.sh"
. "$HERE/lib/config.sh"
. "$HERE/lib/state.sh"
. "$HERE/lib/guards.sh"
. "$HERE/lib/queue.sh"
. "$HERE/lib/agent.sh"
. "$HERE/lib/verify.sh"
. "$HERE/lib/settle.sh"

AP="$PROJECT/.autopilot"
mkdir -p "$AP/logs"
AUTOPILOT_LOG_FILE="$AP/logs/$(date -u +%Y-%m-%d).log"
export AUTOPILOT_LOG_FILE

cfg_load "$AP/config.json" || exit 1
state_init "$AP/state.json"

guard_all "$PROJECT" || exit 0
trap 'guard_unlock "$AP"' EXIT

ISSUE=$(queue_pick)
if [ -z "$ISSUE" ]; then
    log_info "queue is empty"
    exit 0
fi

TITLE=$(queue_field "$ISSUE" title)
BODY=$(queue_field "$ISSUE" body)
log_info "starting #$ISSUE: $TITLE"

AGENT_MODEL=$(cfg_get agent.default_model sonnet)
AGENT_EFFORT=$(cfg_get agent.default_effort low)
export AGENT_MODEL AGENT_EFFORT

WORK_BRANCH=$(cfg_get project.work_branch autopilot/main)
git -C "$PROJECT" checkout -q "$WORK_BRANCH" 2>/dev/null ||
    git -C "$PROJECT" checkout -q -b "$WORK_BRANCH"

PROMPT=$(sed \
    -e "s|{{WORK_BRANCH}}|$WORK_BRANCH|g" \
    -e "s|{{PROJECT_NAME}}|$(cfg_get project.name project)|g" \
    -e "s|{{ISSUE_NUMBER}}|$ISSUE|g" \
    -e "s|{{ISSUE_TITLE}}|$TITLE|g" \
    "$HERE/templates/prompt.tmpl")
PROMPT="$PROMPT

$BODY"

queue_claim "$ISSUE"
state_bump tasks_today >/dev/null

RUN_LOG="$AP/logs/run-$ISSUE-$(date -u +%H%M%S).jsonl"
if agent_run "$PROMPT" "$PROJECT" "$RUN_LOG"; then RC=0; else RC=1; fi

case "$(agent_classify "$RUN_LOG" "$RC")" in
    usage_limit)
        BACKOFF=$(( 900 * ( $(state_get backoff_step 0) + 1 ) ))
        [ "$BACKOFF" -gt 3600 ] && BACKOFF=3600
        state_set_num resume_after "$(( $(date +%s) + BACKOFF ))"
        state_bump backoff_step >/dev/null
        queue_release "$ISSUE"
        git -C "$PROJECT" reset -q --hard
        log_info "usage window closed; retrying in ${BACKOFF}s, #$ISSUE untouched"
        exit 0
        ;;
    task_failure)
        state_bump consecutive_failures >/dev/null
        settle_failure "$PROJECT" "$ISSUE" "agent" "$(tail -c 2000 "$RUN_LOG" 2>/dev/null || true)"
        ;;
    ok)
        state_set_num backoff_step 0
        if verify_run "$PROJECT"; then
            state_set_num consecutive_failures 0
            PREPARE=0
            for L in $(cfg_list autonomy.prepare_only_labels); do
                printf '%s' "$BODY$TITLE" | grep -q "$L" && PREPARE=1
            done
            settle_success "$PROJECT" "$ISSUE" "$TITLE" "$PREPARE"
        else
            state_bump consecutive_failures >/dev/null
            settle_failure "$PROJECT" "$ISSUE" "$VERIFY_FAILED_NAME" "$VERIFY_OUTPUT"
        fi
        ;;
esac

if [ "$(state_get consecutive_failures 0)" -ge "$(cfg_get pacing.circuit_breaker_failures 3)" ]; then
    touch "$AP/STOP"
    log_error "circuit breaker tripped — STOP file created, loop halted"
fi

exit 0
```

- [ ] **Step 4: Run it and confirm it passes**

Run: `chmod +x run-once.sh && sh tests/test_run_once.sh`
Expected: `5 run, 0 failed`

- [ ] **Step 5: Run the whole suite**

Run: `sh tests/run.sh`
Expected: `ALL PASS`

- [ ] **Step 6: Commit**

```bash
git add run-once.sh tests/test_run_once.sh
git commit -m "feat: wire the phases into the entry point

Exit 0 means 'ran, or correctly declined to run'; only a configuration error
exits non-zero, so launchd never sees a normal stand-down as a fault. The lock
is released via trap so no exit path can leave it held."
```

---

### Task 10: Installer and scheduler

**Files:**
- Create: `install-project.sh`, `templates/launchd.plist.tmpl`, `docs/guides/install.md`

**Interfaces:**
- Consumes: `templates/`
- Produces: `install-project.sh <project-path>` — creates `.autopilot/`, seeds config, appends gitignore entries, writes and loads the launchd job.

- [ ] **Step 1: Write the launchd template**

Create `templates/launchd.plist.tmpl`:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>{{LABEL}}</string>
  <key>ProgramArguments</key>
  <array>
    <string>/bin/sh</string>
    <string>{{RUNNER}}</string>
    <string>--project</string>
    <string>{{PROJECT}}</string>
  </array>
  <key>RunAtLoad</key><true/>
  <key>StartInterval</key><integer>{{INTERVAL}}</integer>
  <key>StandardOutPath</key><string>{{PROJECT}}/.autopilot/logs/launchd.out</string>
  <key>StandardErrorPath</key><string>{{PROJECT}}/.autopilot/logs/launchd.err</string>
  <key>EnvironmentVariables</key>
  <dict>
    <key>PATH</key><string>/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:{{HOME}}/.local/bin</string>
  </dict>
</dict>
</plist>
```

`PATH` is set explicitly because launchd jobs do not inherit a login shell's environment — `gh`, `jq`, and `claude` would otherwise not be found, and the failure looks like an empty queue.

- [ ] **Step 2: Write the installer**

Create `install-project.sh`:

```sh
#!/bin/sh
set -eu

HERE=$(cd "$(dirname "$0")" && pwd)
PROJECT=${1:-}
INTERVAL=${2:-2100}

[ -n "$PROJECT" ] || { printf 'usage: install-project.sh <project-path> [interval-seconds]\n' >&2; exit 1; }
PROJECT=$(cd "$PROJECT" && pwd)
[ -d "$PROJECT/.git" ] || { printf 'not a git repository: %s\n' "$PROJECT" >&2; exit 1; }

NAME=$(basename "$PROJECT")
LABEL="com.autopilot.$NAME"
AP="$PROJECT/.autopilot"

mkdir -p "$AP/logs"
if [ ! -f "$AP/config.json" ]; then
    sed "s|CHANGE_ME|$NAME|" "$HERE/templates/config.json" > "$AP/config.json"
    printf 'created %s — edit its verify commands before enabling\n' "$AP/config.json"
fi

for entry in ".autopilot/state.json" ".autopilot/logs/" ".autopilot/STOP" ".autopilot/lock/"; do
    grep -qxF "$entry" "$PROJECT/.gitignore" 2>/dev/null || printf '%s\n' "$entry" >> "$PROJECT/.gitignore"
done

PLIST="$HOME/Library/LaunchAgents/$LABEL.plist"
mkdir -p "$HOME/Library/LaunchAgents"
sed -e "s|{{LABEL}}|$LABEL|g" \
    -e "s|{{RUNNER}}|$HERE/run-once.sh|g" \
    -e "s|{{PROJECT}}|$PROJECT|g" \
    -e "s|{{INTERVAL}}|$INTERVAL|g" \
    -e "s|{{HOME}}|$HOME|g" \
    "$HERE/templates/launchd.plist.tmpl" > "$PLIST"

printf '\nWrote %s\n' "$PLIST"
printf 'Review %s, then enable with:\n' "$AP/config.json"
printf '  launchctl bootstrap gui/$(id -u) %s\n' "$PLIST"
printf 'Pause at any time with:\n'
printf '  touch %s/STOP\n' "$AP"
```

The installer deliberately **does not** load the job. Enabling an unattended process with elevated permissions is the operator's decision, taken after reading the config.

- [ ] **Step 3: Verify the installer against a throwaway repo**

```bash
tmp=$(mktemp -d) && git -C "$tmp" init -q && sh install-project.sh "$tmp"
test -f "$tmp/.autopilot/config.json" && echo "config: ok"
grep -q ".autopilot/state.json" "$tmp/.gitignore" && echo "gitignore: ok"
test -f "$HOME/Library/LaunchAgents/com.autopilot.$(basename "$tmp").plist" && echo "plist: ok"
rm -rf "$tmp" "$HOME/Library/LaunchAgents/com.autopilot.$(basename "$tmp").plist"
```

Expected: three `ok` lines.

- [ ] **Step 4: Write the install guide**

Create `docs/guides/install.md` covering: prerequisites (`gh auth login`, `jq`), running `install-project.sh`, the four config fields that matter, enabling and disabling the launchd job, the STOP switch, where logs land, and how to read a failed run.

- [ ] **Step 5: Commit**

```bash
git add install-project.sh templates/launchd.plist.tmpl docs/guides/install.md
git commit -m "feat: add per-project installer and launchd job

The installer writes the job but does not load it. launchd jobs get no login
shell environment, so PATH is set explicitly in the plist — otherwise gh and
jq are simply absent and the failure presents as a permanently empty queue."
```

---

### Task 11: End-to-end against a real repository

Everything before this ran against stubs. This task is the first time the real `gh` and the real agent are used together, and it is the only proof that matters.

**Files:**
- Create: `docs/reference/observed-behaviour.md`

- [ ] **Step 1: Prepare a throwaway repository**

```bash
gh repo create autopilot-e2e --private --clone
cd autopilot-e2e
git checkout -b autopilot/main && git push -u origin autopilot/main
sh ~/.local/share/autopilot/install-project.sh .
```

Set `verify` to `[{"name":"file-exists","cmd":"test -f HELLO.md"}]` and `project.work_branch` to `autopilot/main`.

- [ ] **Step 2: Create one real issue**

```bash
gh label create autopilot --color 0e8a16 --description "Eligible for unattended execution"
gh issue create --label autopilot --title "Add HELLO.md" \
  --body "Create a file named HELLO.md at the repository root containing a single line: Hello from autopilot.

Verify: test -f HELLO.md"
```

- [ ] **Step 3: Run once, by hand, and watch**

```bash
sh ~/.local/share/autopilot/run-once.sh --project "$(pwd)"
```

Expected: the issue is claimed, the agent creates the file, verification passes, a commit lands with a `Closes #1` trailer, the issue closes, and the working tree is clean.

- [ ] **Step 4: Prove the failure path**

Change `verify` to `[{"name":"impossible","cmd":"false"}]`, reopen the issue, run again.

Expected: the working tree is clean afterwards, **no commit was made**, and the issue carries a failure comment naming `impossible`.

- [ ] **Step 5: Prove the usage-limit path without waiting for a real limit**

Put a stub `claude` earlier on `PATH` that exits non-zero, and confirm the run sets `resume_after` in `state.json`, leaves `consecutive_failures` unchanged, and does not comment on the issue.

```bash
mkdir -p /tmp/stub && printf '#!/bin/sh\nexit 1\n' > /tmp/stub/claude && chmod +x /tmp/stub/claude
PATH=/tmp/stub:$PATH sh ~/.local/share/autopilot/run-once.sh --project "$(pwd)"
jq '{resume_after, consecutive_failures}' .autopilot/state.json
```

Expected: `resume_after` is a future timestamp and `consecutive_failures` is `0`. **If `consecutive_failures` is non-zero, invariant 2 is broken** and the classifier must be fixed before going further.

- [ ] **Step 6: Prove the scheduler**

Load the launchd job, reboot the machine, log in, and confirm a run appears in `.autopilot/logs/` without typing anything.

- [ ] **Step 7: Record what was observed**

Create `docs/reference/observed-behaviour.md` with the date, the exact shapes seen from `gh` and `claude`, and anything that behaved differently from what this plan assumed. **If the real throttled payload was observed at any point, record it verbatim** — it is the one thing the design had to guess at.

- [ ] **Step 8: Commit and clean up**

```bash
git add docs/reference/observed-behaviour.md
git commit -m "docs: record end-to-end observations from the first real run"
gh repo delete autopilot-e2e --yes
```

---

## Self-review notes

**Spec coverage.** Every component in the design maps to a task: guards → 4, queue with dependency resolution → 5, agent with model tiering and the probe classifier → 6, verification gate → 7, settlement with the autonomy ceiling → 8, scheduler → 10, memory layers → carried by the prompt template in 6 and the commit conventions in 8. Two spec elements are deliberately **not** implemented here and are listed under *Out of scope* below.

**Known gap.** The design describes explorer and validator subagents inside a run. The runner does not orchestrate them; they are behaviour the agent chooses inside its own session, driven by the prompt template. If measurement later shows the agent is not using them, that becomes a prompt change, not a runner change.

## Documentation to update

- [ ] `docs/guides/install.md` — created in Task 10
- [ ] `docs/reference/observed-behaviour.md` — created in Task 11
- [ ] `README.md` — change Status from "implementation not started" once Task 11 passes
- [ ] `docs/decisions/0002-json-config-over-yaml.md` — record the deviation flagged above

## Risks

| Risk | Level | Mitigation |
|---|---|---|
| `bash 3.2` silently mis-handles a construct written for bash 4 | High | Global constraint; every script targets `/bin/sh` and tests run under `sh` |
| launchd job cannot find `gh`/`jq`/`claude` and the failure looks like an empty queue | High | `PATH` set explicitly in the plist; Task 11 step 6 exercises the real scheduler |
| The real throttled payload does not match the classifier | Medium | The probe does not parse it; Task 11 step 5 proves the path without waiting for a real throttle |
| An agent commit passes verification but is wrong | Medium | Morning review of overnight commits; the design's validator subagent |
| `queue_pick` is O(open issues) in `gh` calls for dependency checks | Low | Capped at 50 issues per run; revisit only if it becomes slow in practice |

## Out of scope

- Concurrency and worktree isolation. One task per wake, per ADR 0001.
- Linux support. `launchd` only; a systemd timer is a separate plan.
- Any queue backend other than GitHub Issues. The `queue.sh` boundary exists so a swap is possible later, but no second backend is built.
- Automatic tuning of the interval or the daily cap. Both are set by hand after a week of real measurement.
