#!/bin/sh
# Settlement. The only file that writes to git history.

# Every gh call here reports a decision that has already been taken, and most
# of them run after the work is already pushed. So a failure must be said out
# loud and stepped over: never swallowed by `>/dev/null 2>&1`, and never left
# bare, because under run-once.sh's `set -eu` a bare failing gh ended the run
# at the call site (docs/reference/observed-behaviour.md, 2026-08-15) — after
# the commit, with the issue neither commented nor closed.
_settle_gh() {
    _what=$1
    shift
    if _err=$(gh "$@" 2>&1 >/dev/null); then
        return 0
    fi
    log_error "$_what failed: $_err"
    return 1
}

# Checked before every write. The runner has elevated permissions and no
# supervision, so the main branch is protected here rather than by convention.
_settle_guard_branch() {
    _cur=$(git -C "$1" rev-parse --abbrev-ref HEAD 2>/dev/null || printf 'unknown')
    _main=$(cfg_get project.main_branch main)
    if [ "$_cur" = "$_main" ] || [ "$_cur" = "unknown" ]; then
        log_error "refusing to settle on branch [$_cur]"
        return 1
    fi
    return 0
}

settle_success() {
    _root=$1; _num=$2; _title=$3; _prepare_only=$4
    _settle_guard_branch "$_root" || return 1

    # .autopilot/ is excluded from the commit as well as from the clean. Commit
    # it once and the next `git reset --hard` restores state.json to whatever it
    # held then — silently discarding resume_after, the daily counter, and the
    # failure count. The runner would forget its own usage-limit backoff and
    # hammer a closed door.
    git -C "$_root" add -A -- ':(exclude).autopilot'
    if git -C "$_root" diff --cached --quiet; then
        # Not an error: the agent is told to run the project's checks, and an
        # agent that finished the job properly has usually committed already.
        log_info "the agent committed its own work; nothing left to stage"
    else
        git -C "$_root" commit -q -m "$_title

Implements the task described in issue #$_num, which comes from the approved
plan. The project's verification commands passed before this commit was made.

Closes #$_num"
    fi

    # Push whatever HEAD is now, not only what this function committed. When the
    # agent commits its own work there is nothing to stage but there is still
    # something to deliver — and closing an issue whose work never left the
    # machine is the worst outcome available: it reads as done and is not.
    if ! git -C "$_root" push -q origin HEAD 2>/dev/null; then
        log_error "push failed for #$_num — the work is local only, leaving the issue open"
        _settle_gh "reporting the failed push on #$_num" \
            issue comment "$_num" --repo "$AUTOPILOT_REPO" --body "Implementation finished and verification passed, but pushing to the remote failed. The work exists only on the machine that ran this. Leaving this issue open." || true
        _settle_release "$_num"
        return 1
    fi

    _settle_release "$_num"
    if [ "$_prepare_only" -eq 1 ]; then
        if ! _settle_gh "labelling #$_num for human acceptance" \
            issue edit "$_num" --repo "$AUTOPILOT_REPO" --add-label "needs-human"; then
            log_error "#$_num is complete and pushed but carries no needs-human label — the queue can pick it up and redo it; label it by hand"
        fi
        _settle_gh "commenting on #$_num" \
            issue comment "$_num" --repo "$AUTOPILOT_REPO" --body "Implementation complete and the automated checks passed. This task's correctness depends on behaviour a person has to observe, so it stays open for human acceptance." || true
        log_info "#$_num left open for human acceptance"
    else
        _settle_gh "commenting on #$_num" \
            issue comment "$_num" --repo "$AUTOPILOT_REPO" --body "Completed automatically. Verification passed and the work is committed." || true
        if _settle_gh "closing #$_num" issue close "$_num" --repo "$AUTOPILOT_REPO"; then
            log_info "#$_num closed"
        else
            # The work is committed, verified, and on the remote. Saying
            # "closed" here would be the lie this codebase keeps paying for.
            log_error "#$_num could NOT be closed — its work is verified and pushed; close it by hand"
        fi
    fi
    return 0
}

# The claim label is in queue.exclude_labels, so an issue that keeps it is
# invisible to every later run. On a closed issue that is cosmetic; on one left
# open for a human it is the difference between waiting for acceptance and
# being forgotten. queue_release has already logged gh's own message.
_settle_release() {
    queue_release "$1" ||
        log_error "#$1 still carries status:in-progress — an excluded label, so the queue will skip it until someone removes it"
}

# .autopilot/ is excluded from the clean. Without the exclusion the runner
# deletes its own config, state, and logs on the first failed task — after
# which the scheduler keeps firing into a project that no longer has a
# configuration, forever, with nobody watching.
# Resets to where the run began, not to HEAD. The agent is told to run the
# project's checks and normally commits its own work, so by the time
# verification fails HEAD has already moved — and `reset --hard` with no
# argument would discard only the uncommitted remainder, leaving the rejected
# commit in place. That silently turns the verification gate off in exactly the
# case it exists for.
_settle_reset() {
    git -C "$1" reset -q --hard "${2:-HEAD}"
    git -C "$1" clean -qfd -e .autopilot
}

settle_failure() {
    _root=$1; _num=$2; _reason=$3; _detail=$4
    _settle_reset "$_root" "${AUTOPILOT_START_SHA:-HEAD}"
    _settle_gh "reporting the failure on #$_num" \
        issue comment "$_num" --repo "$AUTOPILOT_REPO" --body "Automated attempt failed at: $_reason

\`\`\`
$_detail
\`\`\`

The working tree was reset; nothing was committed." || true
    _settle_release "$_num"
    log_info "#$_num released after failure at $_reason"
}

settle_blocked() {
    _root=$1; _num=$2; _question=$3
    _settle_reset "$_root" "${AUTOPILOT_START_SHA:-HEAD}"
    _settle_gh "asking the blocking question on #$_num" \
        issue comment "$_num" --repo "$AUTOPILOT_REPO" --body "Blocked — this needs a decision the approved plan does not cover:

$_question" || true
    if ! _settle_gh "labelling #$_num blocked" \
        issue edit "$_num" --repo "$AUTOPILOT_REPO" --add-label "blocked"; then
        log_error "#$_num carries no blocked label — the queue will offer it again and the agent will meet the same undecided question; label it by hand"
    fi
    _settle_release "$_num"
    log_info "#$_num blocked pending a decision"
}
