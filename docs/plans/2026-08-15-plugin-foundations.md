# Plugin Foundations Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the runner installable as a Claude Code plugin and usable on a second project, by separating what gets deployed from what stays in the repository and fixing the three defects that make a fresh project fail silently.

**Architecture:** Everything a scheduled run needs moves under `runner/`, so the deployment boundary is structural rather than a list that can drift. The repository root becomes the plugin root, carrying `.claude-plugin/` manifests alongside. Three shell defects are fixed: labels are created at install time, the launchd job label is derived from the git remote instead of the directory name, and a missing label is distinguished from an empty queue.

**Tech Stack:** POSIX shell (`sh`), `git`, `gh`, `jq`. Tests are shell scripts against real throwaway git repositories with `gh` stubbed on `PATH`.

**Spec:** [`docs/design/2026-08-15-skill-layer-design.md`](../design/2026-08-15-skill-layer-design.md)

## Global Constraints

Copied from the spec and from `CLAUDE.md`; every task's requirements implicitly include these.

- POSIX shell only. No runtime dependency beyond `git`, `gh`, `jq`, and the agent CLI.
- No file in `lib/` exceeds ~150 lines.
- `shellcheck` clean before every commit.
- Tests run against a **real throwaway git repository** created under `$TEST_TMP`, never against mocks of git. `gh` and the agent CLI are stubbed with fixture scripts on `PATH`.
- Every function in `lib/` that makes a decision has a test. Functions that only print do not.
- Never write outside the project root or `.autopilot/`.
- The boundary invariant: `grep -rl "skills/" runner/run-once.sh runner/lib/` must stay empty.
- Commits follow Conventional Commits. One commit is one coherent change including its docs.

## Deviation from the spec, and why

The spec says `ctl.sh status` reports a stale runner copy. Implementing that in shell would require recording the plugin's path into new persistent state at deploy time, because `ctl.sh` runs from the deployed copy and has no way to find the plugin.

This plan narrows it: **`ctl.sh status` prints the deployed version, and the comparison moves to the skill**, which already holds both paths — its own plugin base directory and `~/.local/share/autopilot`. No new state file, and the fact lands where both halves of the comparison exist. Task 2 implements the printing; the comparison belongs to the `autopilot-deliver` plan.

---

### Task 1: Move the runner under `runner/`

Everything a scheduled run needs goes in one directory, so deployment can copy a directory rather than a list of paths that drifts from reality. Nothing changes behaviourally; the suite must be as green after as before.

**Files:**
- Move: `run-once.sh`, `lib/`, `ctl.sh`, `install-project.sh`, `templates/` → `runner/`
- Modify: `tests/run.sh:4`
- Modify: `tests/harness.sh:7`

**Interfaces:**
- Consumes: nothing
- Produces: `$REPO_ROOT/runner/` as the location every later task and test refers to. `AUTOPILOT_HOME` continues to mean the directory holding `run-once.sh`, now `<repo>/runner`.

- [ ] **Step 1: Record the current suite result to compare against**

Run: `sh tests/run.sh`
Expected: ends with `ALL PASS`. If it does not, stop — this task's only proof is that the result is unchanged.

- [ ] **Step 2: Move the files with git so history follows them**

```bash
cd ~/.local/share/autopilot
mkdir -p runner
git mv run-once.sh lib ctl.sh install-project.sh templates runner/
```

- [ ] **Step 3: Point the test runner at the new location**

`tests/run.sh` line 4 currently computes the repository root and calls it `AUTOPILOT_HOME`. Those are now two different directories. Replace line 4:

```sh
AUTOPILOT_HOME=$(cd ../runner && pwd)
```

- [ ] **Step 4: Point the harness at the new location**

`tests/harness.sh` line 7 exports `REPO_ROOT`, which tests use to source library files. Keep `REPO_ROOT` meaning the repository, and add the runner directory beside it so tests do not each write `"$REPO_ROOT/runner"`:

```sh
REPO_ROOT=$(cd "$(dirname "$0")/.." && pwd)
RUNNER_ROOT="$REPO_ROOT/runner"
export REPO_ROOT RUNNER_ROOT
```

- [ ] **Step 5: Update every test that sources a library or script by path**

Search and fix:

```bash
grep -rn '\$REPO_ROOT/\(lib\|run-once\|ctl\|install-project\|templates\)' tests/
```

Each hit becomes `$RUNNER_ROOT/...`. Do not change `$REPO_ROOT` where a test genuinely means the repository.

- [ ] **Step 6: Run the suite and confirm the result is identical**

Run: `sh tests/run.sh`
Expected: `ALL PASS`, same test count as Step 1.

- [ ] **Step 7: Confirm shellcheck is still clean**

Run: `sh scripts/lint-shell.sh 2>/dev/null || shellcheck runner/*.sh runner/lib/*.sh tests/*.sh`
Expected: no output, exit 0.

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "refactor: move the runner under runner/

Deployment copies a directory rather than a list of paths. A list drifts
from what the runner actually needs; a directory cannot.

Behaviour is unchanged and the suite result is identical."
```

---

### Task 2: `runner/VERSION`, printed by `ctl.sh status`

A deployed copy has no git history, so the only way to know which build is running unsupervised on the machine is a file it carries.

**Files:**
- Create: `runner/VERSION`
- Modify: `runner/ctl.sh` (the `status` branch, currently lines 58-76)
- Test: `tests/test_ctl.sh` (create)

**Interfaces:**
- Consumes: `RUNNER_ROOT` from Task 1
- Produces: `runner/VERSION` holding a bare semver string with a trailing newline. `ctl.sh status` prints a line matching `^runner:   ` followed by that string.

- [ ] **Step 1: Write the failing test**

Create `tests/test_ctl.sh`:

```sh
#!/bin/sh
. "$(dirname "$0")/harness.sh"

# ctl.sh must never touch the operator's real launchd domain from a test.
AUTOPILOT_PLIST_DIR="$TEST_TMP/jobs"
export AUTOPILOT_PLIST_DIR
mkdir -p "$AUTOPILOT_PLIST_DIR"

stub_launchctl_absent

repo="$TEST_TMP/proj"
mkdir -p "$repo"
git -C "$repo" init -q

out=$(sh "$RUNNER_ROOT/ctl.sh" status "$repo" 2>&1)

assert_contains "$out" "runner:" "status names the runner version"
assert_contains "$out" "$(cat "$RUNNER_ROOT/VERSION")" "status prints the deployed version"

finish
```

Add the `launchctl` stub to `tests/harness.sh`, beside the existing stubs:

```sh
# ctl.sh calls launchctl print to decide whether a job is loaded. A test must
# never consult the operator's real launchd domain, so the stub reports every
# job as absent.
stub_launchctl_absent() {
    cat > "$STUB_BIN/launchctl" <<'STUB'
#!/bin/sh
exit 1
STUB
    chmod +x "$STUB_BIN/launchctl"
}
```

- [ ] **Step 2: Run the test and watch it fail**

Run: `sh tests/test_ctl.sh`
Expected: FAIL — `status names the runner version` is missing, because `ctl.sh` prints no such line and `runner/VERSION` does not exist.

- [ ] **Step 3: Create the version file**

```bash
printf '0.1.0\n' > runner/VERSION
```

- [ ] **Step 4: Print it from `ctl.sh status`**

In `runner/ctl.sh`, inside the `status)` branch, after the line printing `job file:`, add:

```sh
        _here=$(cd "$(dirname "$0")" && pwd)
        if [ -f "$_here/VERSION" ]; then
            printf 'runner:   %s\n' "$(cat "$_here/VERSION")"
        else
            printf 'runner:   unknown — no VERSION file beside ctl.sh\n'
        fi
```

- [ ] **Step 5: Run the test and confirm it passes**

Run: `sh tests/test_ctl.sh`
Expected: both assertions `ok`.

- [ ] **Step 6: Run the whole suite**

Run: `sh tests/run.sh`
Expected: `ALL PASS`, one more test file than before.

- [ ] **Step 7: Commit**

```bash
git add runner/VERSION runner/ctl.sh tests/test_ctl.sh tests/harness.sh
git commit -m "feat: give the deployed runner a version it can state

A deployed copy carries no git history, so the build running unsupervised
on the machine was unidentifiable. ctl.sh status now names it.

Adds the first test file for ctl.sh, with a launchctl stub so no test can
reach the operator's real launchd domain."
```

---

### Task 3: `runner/lib/label.sh` — derive the job label from the remote

`install-project.sh:26` builds `com.autopilot.$(basename "$PROJECT")`. Two projects whose directories share a name produce one label, and the second install silently overwrites the first's plist.

**Files:**
- Create: `runner/lib/label.sh`
- Test: `tests/test_label.sh` (create)

**Interfaces:**
- Consumes: nothing
- Produces: `label_for_project <project-dir>` prints the launchd label on stdout and returns 0. Format is `com.autopilot.<owner>-<repo>` when an `origin` remote parses, and `com.autopilot.<dirname>-<digest>` otherwise. Task 4 is the only consumer.

- [ ] **Step 1: Write the failing test**

Create `tests/test_label.sh`:

```sh
#!/bin/sh
. "$(dirname "$0")/harness.sh"
. "$RUNNER_ROOT/lib/label.sh"

mk() {
    mkdir -p "$1"
    git -C "$1" init -q
    [ -n "${2:-}" ] && git -C "$1" remote add origin "$2"
    return 0
}

# The defect this file exists to fix: same directory name, different projects.
mk "$TEST_TMP/work/api"  "https://github.com/acme/api.git"
mk "$TEST_TMP/side/api"  "git@github.com:other/api.git"

assert_eq "com.autopilot.acme-api" "$(label_for_project "$TEST_TMP/work/api")" \
    "an https remote yields owner-repo"
assert_eq "com.autopilot.other-api" "$(label_for_project "$TEST_TMP/side/api")" \
    "an ssh remote yields owner-repo"

# A project with no remote still has to be distinguishable from another one
# with the same directory name, or the collision simply moves.
mk "$TEST_TMP/one/local"
mk "$TEST_TMP/two/local"
a=$(label_for_project "$TEST_TMP/one/local")
b=$(label_for_project "$TEST_TMP/two/local")

assert_contains "$a" "com.autopilot.local-" "no remote falls back to the directory name"
assert_eq "0" "$([ "$a" = "$b" ] && echo 1 || echo 0)" \
    "two remoteless projects with one name get different labels"

finish
```

- [ ] **Step 2: Run the test and watch it fail**

Run: `sh tests/test_label.sh`
Expected: FAIL — `label.sh` does not exist, so sourcing it aborts the script.

- [ ] **Step 3: Write the implementation**

Create `runner/lib/label.sh`:

```sh
#!/bin/sh
# The launchd job label. install-project.sh writes a plist under this name and
# ctl.sh manages the job under it; when the two disagree, ctl.sh quietly
# operates on a job the installer never wrote.

# Derived from the origin remote, because a basename alone collides:
# ~/work/api and ~/side/api produce one label, and the second install
# overwrites the first's plist with no warning.
label_for_project() {
    _dir=$1
    _url=$(git -C "$_dir" remote get-url origin 2>/dev/null || printf '')
    if [ -n "$_url" ]; then
        _slug=$(printf '%s' "$_url" \
            | sed -e 's|^git@[^:]*:||' -e 's|^https\{0,1\}://[^/]*/||' -e 's|\.git$||' \
            | tr '/' '-')
        case "$_slug" in
            *-*) printf 'com.autopilot.%s' "$_slug"; return 0 ;;
        esac
    fi
    # No remote, or an origin this cannot parse. The directory name alone would
    # reintroduce the collision, so a digest of the absolute path joins it.
    # cksum is POSIX; shasum and md5 are not portable enough to rely on.
    _abs=$(cd "$_dir" && pwd)
    _digest=$(printf '%s' "$_abs" | cksum | cut -d' ' -f1)
    printf 'com.autopilot.%s-%s' "$(basename "$_abs")" "$_digest"
}
```

- [ ] **Step 4: Run the test and confirm it passes**

Run: `sh tests/test_label.sh`
Expected: four assertions `ok`.

- [ ] **Step 5: Confirm shellcheck is clean**

Run: `shellcheck runner/lib/label.sh tests/test_label.sh`
Expected: no output.

- [ ] **Step 6: Commit**

```bash
git add runner/lib/label.sh tests/test_label.sh
git commit -m "feat: derive the job label from the remote, not the directory

~/work/api and ~/side/api produced one launchd label, so installing the
second overwrote the first project's plist and left it managing a job
pointing at the wrong repository.

Remoteless projects fall back to the directory name plus a digest of the
absolute path, so the collision does not simply move."
```

---

### Task 4: Both scripts derive the label through `label.sh`

ADR-0003 already records that `install-project.sh` and `ctl.sh` are "both wrong together or right together". Two independent copies of the derivation is exactly the failure it anticipated.

**Files:**
- Modify: `runner/install-project.sh:26` and the plist path built from it
- Modify: `runner/ctl.sh:22-23,28`
- Test: `tests/test_install.sh` (extend)

**Interfaces:**
- Consumes: `label_for_project` from Task 3
- Produces: nothing new. After this task, no file computes a label by any other means.

- [ ] **Step 1: Write the failing test**

Append to `tests/test_install.sh`, before its `finish` call:

```sh
# Same directory name, different repositories. Before label.sh, the second
# install overwrote the first's plist and both projects shared one job.
one="$TEST_TMP/alpha/svc"; two="$TEST_TMP/beta/svc"
mkdir -p "$one" "$two"
git -C "$one" init -q; git -C "$one" remote add origin https://github.com/alpha/svc.git
git -C "$two" init -q; git -C "$two" remote add origin https://github.com/beta/svc.git

sh "$RUNNER_ROOT/install-project.sh" "$one" 1800 >/dev/null 2>&1
sh "$RUNNER_ROOT/install-project.sh" "$two" 1800 >/dev/null 2>&1

assert_eq "2" "$(find "$AUTOPILOT_PLIST_DIR" -name 'com.autopilot.*.plist' | wc -l | tr -d ' ')" \
    "two projects with one directory name get two plists"

# ctl.sh must find the same job the installer wrote, or it manages nothing.
assert_contains "$(sh "$RUNNER_ROOT/ctl.sh" status "$one" 2>&1)" "alpha-svc" \
    "ctl.sh derives the same label the installer used"
```

- [ ] **Step 2: Run the test and watch it fail**

Run: `sh tests/test_install.sh`
Expected: FAIL — `two projects with one directory name get two plists` reports `1`, because the second install overwrote the first.

- [ ] **Step 3: Source `label.sh` in the installer and use it**

In `runner/install-project.sh`, after `HERE=` is computed, add:

```sh
. "$HERE/lib/label.sh"
```

Replace the `LABEL=` assignment (line 26) with:

```sh
LABEL=$(label_for_project "$PROJECT")
```

`NAME` stays as it is — it is used for the config's project name and for human-facing messages, not for the label.

- [ ] **Step 4: Source `label.sh` in the control script and use it**

In `runner/ctl.sh`, after `PROJECT` is canonicalised, replace the `LABEL=` assignment with:

```sh
. "$(cd "$(dirname "$0")" && pwd)/lib/label.sh"
LABEL=$(label_for_project "$PROJECT")
```

- [ ] **Step 5: Run the test and confirm it passes**

Run: `sh tests/test_install.sh`
Expected: all assertions `ok`, including the two new ones.

- [ ] **Step 6: Run the whole suite**

Run: `sh tests/run.sh`
Expected: `ALL PASS`.

- [ ] **Step 7: Commit**

```bash
git add runner/install-project.sh runner/ctl.sh tests/test_install.sh
git commit -m "fix: one derivation of the job label, used by both scripts

ADR-0003 recorded that install-project.sh and ctl.sh are both wrong
together or right together. They each built the label themselves, which
is how they came to disagree."
```

---

### Task 5: Create the labels the runner queries

A fresh repository has none of them, and `lib/queue.sh` cannot tell a missing label from an empty queue — so the loop reports no work, forever, with nothing saying why.

**Files:**
- Modify: `runner/install-project.sh` (after the config is written)
- Test: `tests/test_install.sh` (extend)

**Interfaces:**
- Consumes: nothing
- Produces: nine labels in the project's GitHub repository. Names are fixed and are the same strings `templates/config.json` and `run-once.sh:57-62` already rely on.

- [ ] **Step 1: Write the failing test**

Append to `tests/test_install.sh`:

```sh
# gh must be stubbed: a test may never create labels in a real repository.
cat > "$STUB_BIN/gh" <<'STUB'
#!/bin/sh
printf '%s\n' "$*" >> "$GH_CALLS"
exit 0
STUB
chmod +x "$STUB_BIN/gh"
GH_CALLS="$TEST_TMP/gh-calls"; export GH_CALLS
: > "$GH_CALLS"

fresh="$TEST_TMP/fresh/repo"
mkdir -p "$fresh"
git -C "$fresh" init -q
git -C "$fresh" remote add origin https://github.com/acme/fresh.git

sh "$RUNNER_ROOT/install-project.sh" "$fresh" 1800 >/dev/null 2>&1

calls=$(cat "$GH_CALLS")
assert_contains "$calls" "label create autopilot"          "the eligibility label is created"
assert_contains "$calls" "label create needs-human"        "the prepare-only label is created"
assert_contains "$calls" "label create blocked"            "the blocked label is created"
assert_contains "$calls" "label create status:in-progress" "the claim label is created"
assert_contains "$calls" "label create model:opus"         "the model labels are created"
assert_contains "$calls" "label create effort:high"        "the effort labels are created"
assert_contains "$calls" "--repo acme/fresh"               "labels are created in the project's own repository"
```

- [ ] **Step 2: Run the test and watch it fail**

Run: `sh tests/test_install.sh`
Expected: FAIL — every `label create` assertion is missing; the installer makes no `gh` calls at all.

- [ ] **Step 3: Create the labels in the installer**

In `runner/install-project.sh`, after the `.gitignore` block and before the plist is written, add:

```sh
# The runner queries these by name and cannot tell a missing label from an
# empty queue, so a project without them reports no work forever. Creating
# them is idempotent: an existing label is not an error worth stopping for.
SLUG=$(printf '%s' "$(git -C "$PROJECT" remote get-url origin 2>/dev/null || printf '')" \
    | sed -e 's|^git@[^:]*:||' -e 's|^https\{0,1\}://[^/]*/||' -e 's|\.git$||')
if [ -n "$SLUG" ]; then
    printf 'creating labels in %s\n' "$SLUG"
    while IFS='|' read -r lname lcolour ldesc; do
        [ -n "$lname" ] || continue
        gh label create "$lname" --repo "$SLUG" --color "$lcolour" \
            --description "$ldesc" >/dev/null 2>&1 ||
            printf '  %s already exists\n' "$lname"
    done <<'LABELS'
autopilot|0e8a16|Eligible for unattended execution
needs-human|fbca04|Correctness needs a person to observe it; autopilot must not close
blocked|d93f0b|Waiting on a human decision
status:in-progress|1d76db|Claimed by a run
model:sonnet|c5def5|Run with Sonnet
model:opus|d4c5f9|Run with Opus
effort:low|ededed|Mechanical work
effort:medium|d9d9d9|Moderate reasoning
effort:high|bfbfbf|Hard reasoning
LABELS
else
    printf 'no origin remote — skipping label creation\n'
fi
```

- [ ] **Step 4: Run the test and confirm it passes**

Run: `sh tests/test_install.sh`
Expected: all seven new assertions `ok`.

- [ ] **Step 5: Confirm a second install is clean**

Run: `sh tests/test_install.sh` again with the same stub in place, then read the stub log:

```bash
grep -c 'label create autopilot' "$TEST_TMP/gh-calls"
```
Expected: creating labels twice produces no failure and no error output — the `|| printf` branch absorbs it.

- [ ] **Step 6: Run the whole suite and shellcheck**

Run: `sh tests/run.sh && shellcheck runner/install-project.sh`
Expected: `ALL PASS`, no shellcheck output.

- [ ] **Step 7: Commit**

```bash
git add runner/install-project.sh tests/test_install.sh
git commit -m "feat: create the labels the runner queries

A fresh project had none of them, and queue.sh reports a missing label
and an empty queue identically. Installing then produced a loop that
stood down every tick and never said why."
```

---

### Task 6: Tell a missing label apart from an empty queue

`lib/queue.sh:28-30` ends in `2>/dev/null || printf '[]'`, which turns every failure into "no work to do". This is the silent half of the defect Task 5 fixed the loud half of.

**Files:**
- Modify: `runner/lib/queue.sh:26-31`
- Test: `tests/test_queue.sh` (extend)

**Interfaces:**
- Consumes: `log_error` from `lib/log.sh`
- Produces: `queue_candidates` still prints a JSON array on stdout and now returns non-zero when `gh` failed. `queue_load` is unchanged and still normalises anything unparseable to `[]`.

- [ ] **Step 1: Write the failing test**

Append to `tests/test_queue.sh`:

```sh
# gh failing and there being no work are different facts. Until now both
# arrived as [], so a project whose labels were never created looked exactly
# like a project with nothing to do.
cat > "$STUB_BIN/gh" <<'STUB'
#!/bin/sh
echo "could not add label: 'autopilot' not found" >&2
exit 1
STUB
chmod +x "$STUB_BIN/gh"

AUTOPILOT_LOG_FILE="$TEST_TMP/queue-fail.log"
export AUTOPILOT_LOG_FILE
: > "$AUTOPILOT_LOG_FILE"

out=$(queue_candidates); rc=$?

assert_eq "[]" "$out" "a failed lookup still yields a parseable empty array"
assert_eq "1" "$rc"   "a failed lookup reports failure to its caller"
assert_contains "$(cat "$AUTOPILOT_LOG_FILE")" "not found" \
    "the log names the reason rather than reporting an empty queue"
```

- [ ] **Step 2: Run the test and watch it fail**

Run: `sh tests/test_queue.sh`
Expected: FAIL — `rc` is `0` and the log is empty, because the current code discards both the status and the message.

- [ ] **Step 3: Rewrite `queue_candidates`**

Replace `runner/lib/queue.sh` lines 26-31 with:

```sh
queue_candidates() {
    _label=$(cfg_get queue.ready_label autopilot)
    if _out=$(gh issue list --repo "$AUTOPILOT_REPO" --state open --limit 50 \
            --json number,title,body,labels,milestone \
            --label "$_label" 2>&1); then
        printf '%s' "$_out"
        return 0
    fi
    # Reporting this as an empty queue is how a freshly installed project
    # stands down every tick for a week without anyone learning why.
    log_error "cannot read the queue for $AUTOPILOT_REPO: $_out"
    printf '[]'
    return 1
}
```

- [ ] **Step 4: Run the test and confirm it passes**

Run: `sh tests/test_queue.sh`
Expected: three new assertions `ok`, and every pre-existing assertion in the file still `ok`.

- [ ] **Step 5: Run the whole suite and shellcheck**

Run: `sh tests/run.sh && shellcheck runner/lib/queue.sh`
Expected: `ALL PASS`, no shellcheck output.

- [ ] **Step 6: Record the behaviour that was paid for**

Append to `docs/reference/observed-behaviour.md`:

```markdown
## `gh issue list` against a label that does not exist (2026-08-15)

`gh issue list --label <name>` on a repository where `<name>` was never created
exits non-zero and writes the reason to stderr. The previous
`2>/dev/null || printf '[]'` swallowed both, so a project installed without its
labels reported an empty queue on every tick indefinitely — indistinguishable
from having finished all its work.

Labels are now created by `install-project.sh`, and `queue_candidates` returns
non-zero and logs the message rather than reporting no work.
```

- [ ] **Step 7: Commit**

```bash
git add runner/lib/queue.sh tests/test_queue.sh docs/reference/observed-behaviour.md
git commit -m "fix: distinguish a failed queue lookup from an empty queue

2>/dev/null || printf '[]' turned every gh failure into no work to do.
A project whose labels were never created stood down on every tick and
said nothing, which is the failure mode hardest to notice."
```

---

### Task 7: Plugin manifests

The repository root becomes the plugin root, so `/plugin install` delivers the runner source and, once the skill plans land, the skills beside it.

**Files:**
- Create: `.claude-plugin/plugin.json`
- Create: `.claude-plugin/marketplace.json`
- Test: `tests/test_plugin.sh` (create)

**Interfaces:**
- Consumes: `runner/VERSION` from Task 2
- Produces: a plugin named `autopilot` whose `version` field equals `runner/VERSION`. Later plans add `skills/` beside `runner/`; no manifest change is needed when they do.

- [ ] **Step 1: Write the failing test**

Create `tests/test_plugin.sh`:

```sh
#!/bin/sh
. "$(dirname "$0")/harness.sh"

pj="$REPO_ROOT/.claude-plugin/plugin.json"
mj="$REPO_ROOT/.claude-plugin/marketplace.json"

assert_eq "1" "$([ -f "$pj" ] && echo 1 || echo 0)" "the plugin manifest exists"
assert_eq "1" "$([ -f "$mj" ] && echo 1 || echo 0)" "the marketplace manifest exists"

assert_eq "0" "$(jq -e . "$pj" >/dev/null 2>&1; echo $?)" "the plugin manifest is valid JSON"
assert_eq "0" "$(jq -e . "$mj" >/dev/null 2>&1; echo $?)" "the marketplace manifest is valid JSON"

assert_eq "autopilot" "$(jq -r .name "$pj")" "the plugin is named autopilot"

# One version, or the deployed copy and the plugin disagree about what it is.
assert_eq "$(cat "$REPO_ROOT/runner/VERSION")" "$(jq -r .version "$pj")" \
    "plugin.json agrees with runner/VERSION"
assert_eq "$(cat "$REPO_ROOT/runner/VERSION")" \
    "$(jq -r '.plugins[0].version' "$mj")" \
    "marketplace.json agrees with runner/VERSION"

finish
```

- [ ] **Step 2: Run the test and watch it fail**

Run: `sh tests/test_plugin.sh`
Expected: FAIL on the first assertion — neither manifest exists.

- [ ] **Step 3: Write the plugin manifest**

Create `.claude-plugin/plugin.json`:

```json
{
  "name": "autopilot",
  "description": "Unattended delivery loop: takes approved plans to reviewed GitHub issues, runs them one at a time, and gates every commit on the project's own verification commands",
  "version": "0.1.0",
  "author": {
    "name": "thangtran"
  },
  "homepage": "https://github.com/tmthang86/autopilot",
  "repository": "https://github.com/tmthang86/autopilot",
  "license": "MIT",
  "keywords": [
    "autopilot",
    "unattended",
    "delivery",
    "github-issues",
    "launchd"
  ]
}
```

- [ ] **Step 4: Write the marketplace manifest**

Create `.claude-plugin/marketplace.json`:

```json
{
  "name": "autopilot",
  "description": "The autopilot delivery runner and its operator skills",
  "owner": {
    "name": "thangtran"
  },
  "plugins": [
    {
      "name": "autopilot",
      "description": "Unattended delivery loop: takes approved plans to reviewed GitHub issues, runs them one at a time, and gates every commit on the project's own verification commands",
      "version": "0.1.0",
      "source": "./"
    }
  ]
}
```

- [ ] **Step 5: Run the test and confirm it passes**

Run: `sh tests/test_plugin.sh`
Expected: seven assertions `ok`.

- [ ] **Step 6: Run the whole suite**

Run: `sh tests/run.sh`
Expected: `ALL PASS`.

- [ ] **Step 7: Add the boundary test**

Append to `tests/test_run_once.sh`:

```sh
# Skills run when a person is present; the loop runs when nobody is. A loop
# that could invoke a skill would reach the one capability reserved for
# supervised moments.
assert_eq "" "$(grep -rl 'skills/' "$RUNNER_ROOT/run-once.sh" "$RUNNER_ROOT/lib/" 2>/dev/null)" \
    "the unattended loop never reaches into the skill layer"
```

- [ ] **Step 8: Run the suite once more and commit**

```bash
sh tests/run.sh
git add .claude-plugin tests/test_plugin.sh tests/test_run_once.sh
git commit -m "feat: package the repository as a Claude Code plugin

The repository root becomes the plugin root, so /plugin install delivers
runner/ and, once the skill plans land, skills/ beside it. A test asserts
one version across plugin.json, marketplace.json, and runner/VERSION.

Adds the boundary test: the unattended loop may never reach into the
skill layer, because a scheduled run has no operator."
```

---

### Task 8: Documentation

`CLAUDE.md` §3 and the install guide both describe a system that no longer exists after Tasks 1-7.

**Files:**
- Modify: `CLAUDE.md` §3 and §5
- Modify: `docs/guides/install.md`

**Interfaces:**
- Consumes: everything above
- Produces: nothing executable

- [ ] **Step 1: Update `CLAUDE.md` §3**

The list of project-specific concerns gains a fifth entry, and portability now includes preparing the queue. Add after the fourth bullet:

```markdown
- **intent** — how a task names the documents that authorise it

Portability now includes queue preparation: `install-project.sh` creates the
labels the runner queries, so a project is not required to have been prepared
by hand before the runner is pointed at it.
```

- [ ] **Step 2: Record `docs/design/` in `CLAUDE.md` §5**

Add to the documentation list:

```markdown
- `docs/design/` — validated designs, written before the plans that implement them
```

- [ ] **Step 3: Remove the stale precondition from the install guide**

In `docs/guides/install.md`, the "Before you start" section states that work must already exist as labelled issues. Replace that sentence with:

```markdown
The project must be a git repository with an `origin` remote. The installer
creates the labels the runner queries; producing the issues themselves is the
job of the `autopilot-deliver` skill.
```

- [ ] **Step 4: Document the new layout in the install guide**

Add a section after "Install":

```markdown
## What is installed where

The repository is the plugin and the source. `runner/` is what gets deployed to
`~/.local/share/autopilot/`, and it is the only part a scheduled run needs:

    run-once.sh · lib/ · ctl.sh · install-project.sh · templates/ · VERSION

`docs/`, `tests/`, and git history stay in the plugin. The deployed copy is
never edited in place — fix it in the repository and deliver the fix with
`/plugin update`. `ctl.sh status` names the deployed version.
```

- [ ] **Step 5: Confirm nothing else in the docs is now false**

Run: `grep -rn 'install-project.sh\|run-once.sh\|lib/' docs/ README.md`
Read each hit and correct any path that no longer resolves after Task 1.

- [ ] **Step 6: Commit**

```bash
git add CLAUDE.md docs/guides/install.md README.md
git commit -m "docs: describe the plugin layout and the prepared queue

CLAUDE.md §3 listed four project-specific concerns and claimed the runner
knows nothing about preparing a project. Both changed. The install guide
still required issues to exist before installing, which was the precondition
this work exists to remove."
```

---

## Self-Review

**Spec coverage.** Walked each section of `docs/design/2026-08-15-skill-layer-design.md`:

| Spec section | Task |
|---|---|
| Architecture — `runner/` split, deployed copy contents | 1 |
| `VERSION` and staleness | 2, narrowed (see *Deviation*); comparison deferred to the deliver plan |
| Boundary invariant, checkable by grep | 7, step 7 |
| Distribution as a plugin | 7 |
| The deployed copy is not the source repository | 1, 8 |
| Label creation | 5 |
| `lib/label.sh`, collision fix | 3, 4 |
| `queue_candidates` distinguishing missing label | 6 |
| Intent binding | **Not here** — separate plan, committed as `211d303` |
| `autopilot-deliver`, `autopilot-review` | **Not here** — separate plans, blocked on this one |
| Effect on existing documents | 8 |

**Placeholder scan.** No `TBD`, no "add error handling", no "similar to Task N". Every code step carries the actual code. The one place a step says "search and fix" (Task 1 step 5) gives the exact `grep` and the exact substitution rule, because the hit list depends on the working tree at that moment.

**Type consistency.** `label_for_project` is defined in Task 3 and consumed under that name in Task 4. `RUNNER_ROOT` is introduced in Task 1 step 4 and used by Tasks 2, 3, 4, 5, 7. `stub_launchctl_absent` is defined in Task 2 and not relied on elsewhere. `queue_candidates` keeps its name and stdout contract in Task 6; only its exit status is new, and `queue_load` — its only caller — ignores exit status, so nothing downstream breaks.

**One risk this plan carries.** Task 1 moves five paths at once. If the suite is not green at step 1, everything after it is unverifiable, which is why step 1 exists and why it says to stop.
