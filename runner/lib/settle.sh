#!/bin/sh
# Settlement. The only file that writes to git history.

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
        gh issue comment "$_num" --repo "$AUTOPILOT_REPO" --body "Implementation finished and verification passed, but pushing to the remote failed. The work exists only on the machine that ran this. Leaving this issue open." >/dev/null 2>&1
        queue_release "$_num"
        return 1
    fi

    queue_release "$_num"
    if [ "$_prepare_only" -eq 1 ]; then
        gh issue edit "$_num" --repo "$AUTOPILOT_REPO" --add-label "needs-human" >/dev/null 2>&1
        gh issue comment "$_num" --repo "$AUTOPILOT_REPO" --body "Implementation complete and the automated checks passed. This task's correctness depends on behaviour a person has to observe, so it stays open for human acceptance." >/dev/null 2>&1
        log_info "#$_num left open for human acceptance"
    else
        gh issue comment "$_num" --repo "$AUTOPILOT_REPO" --body "Completed automatically. Verification passed and the work is committed." >/dev/null 2>&1
        gh issue close "$_num" --repo "$AUTOPILOT_REPO" >/dev/null 2>&1
        log_info "#$_num closed"
    fi
    return 0
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
    gh issue comment "$_num" --repo "$AUTOPILOT_REPO" --body "Automated attempt failed at: $_reason

\`\`\`
$_detail
\`\`\`

The working tree was reset; nothing was committed." >/dev/null 2>&1
    queue_release "$_num"
    log_info "#$_num released after failure at $_reason"
}

settle_blocked() {
    _root=$1; _num=$2; _question=$3
    _settle_reset "$_root" "${AUTOPILOT_START_SHA:-HEAD}"
    gh issue comment "$_num" --repo "$AUTOPILOT_REPO" --body "Blocked — this needs a decision the approved plan does not cover:

$_question" >/dev/null 2>&1
    gh issue edit "$_num" --repo "$AUTOPILOT_REPO" --add-label "blocked" >/dev/null 2>&1
    queue_release "$_num"
    log_info "#$_num blocked pending a decision"
}
