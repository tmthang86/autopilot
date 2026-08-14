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
        log_warn "nothing to commit for #$_num"
    else
        git -C "$_root" commit -q -m "$_title

Implements the task described in issue #$_num, which comes from the approved
plan. The project's verification commands passed before this commit was made.

Closes #$_num"
        git -C "$_root" push -q origin HEAD 2>/dev/null || log_warn "push failed; the commit is local"
    fi

    queue_release "$_num"
    if [ "$_prepare_only" -eq 1 ]; then
        gh issue edit "$_num" --add-label "needs-human" >/dev/null 2>&1
        gh issue comment "$_num" --body "Implementation complete and the automated checks passed. This task's correctness depends on behaviour a person has to observe, so it stays open for human acceptance." >/dev/null 2>&1
        log_info "#$_num left open for human acceptance"
    else
        gh issue comment "$_num" --body "Completed automatically. Verification passed and the work is committed." >/dev/null 2>&1
        gh issue close "$_num" >/dev/null 2>&1
        log_info "#$_num closed"
    fi
    return 0
}

# .autopilot/ is excluded from the clean. Without the exclusion the runner
# deletes its own config, state, and logs on the first failed task — after
# which the scheduler keeps firing into a project that no longer has a
# configuration, forever, with nobody watching.
_settle_reset() {
    git -C "$1" reset -q --hard
    git -C "$1" clean -qfd -e .autopilot
}

settle_failure() {
    _root=$1; _num=$2; _reason=$3; _detail=$4
    _settle_reset "$_root"
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
    _settle_reset "$_root"
    gh issue comment "$_num" --body "Blocked — this needs a decision the approved plan does not cover:

$_question" >/dev/null 2>&1
    gh issue edit "$_num" --add-label "blocked" >/dev/null 2>&1
    queue_release "$_num"
    log_info "#$_num blocked pending a decision"
}
