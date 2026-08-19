//! Settlement: the only module that writes git history and closes issues.
//!
//! Work happens on `autopilot/task-<n>`; a green verify rebases onto
//! `autopilot/main`, verifies again, and fast-forwards. A pause commits a
//! `WIP:` commit to the task branch and pushes it — invariant 1 is *no
//! unverified work reaches `autopilot/main`*, not *nothing is committed*.
//! Every rejection path rewinds to the recorded START_SHA, never `HEAD`.

use std::path::Path;

pub fn current_branch(project: &Path) -> String {
    crate::git::run(project, &["rev-parse", "--abbrev-ref", "HEAD"], 60)
        .map(|o| o.stdout.trim().to_string())
        .unwrap_or_else(|_| "unknown".into())
}

pub fn start_sha(project: &Path) -> String {
    crate::git::run(project, &["rev-parse", "HEAD"], 60)
        .map(|o| o.stdout.trim().to_string())
        .unwrap_or_default()
}

pub fn reset(project: &Path, sha: &str) {
    let _ = crate::git::run(project, &["reset", "-q", "--hard", sha], 120);
    let _ = crate::git::run(project, &["clean", "-qfd", "-e", ".autopilot"], 120);
}

/// Checked before every write, not assumed (Rule Zero): refuses if HEAD is on
/// the project's real main branch, or the branch could not be read at all.
fn guard_not_main(project: &Path, main_branch: &str) -> bool {
    let cur = current_branch(project);
    if cur == main_branch || cur == "unknown" {
        crate::log::error(&format!(
            "refusing to write: current branch is [{cur}], not a task or accumulation branch"
        ));
        return false;
    }
    true
}

/// Creates `branch` from `start_point` if it does not already exist. The
/// accumulation branch has no install-time step that creates it — a fresh
/// project reaches this before its first task, same as the shell runner's
/// `checkout || checkout -b` did for `work_branch`.
pub fn ensure_branch(project: &Path, branch: &str, start_point: &str) -> bool {
    let exists = crate::git::run(project, &["rev-parse", "--verify", "--quiet", branch], 30)
        .map(|o| o.status == 0)
        .unwrap_or(false);
    if exists {
        return true;
    }
    crate::git::run(project, &["branch", branch, start_point], 60)
        .map(|o| o.status == 0)
        .unwrap_or(false)
}

/// Ensures the accumulation branch exists, then checks out a fresh task
/// branch from it.
pub fn start_task_branch(project: &Path, issue: u64, main_branch: &str, work_branch: &str) -> bool {
    if !ensure_branch(project, work_branch, main_branch) {
        return false;
    }
    let branch = format!("autopilot/task-{issue}");
    checkout(project, "-B", &branch, work_branch)
}

pub fn checkout(project: &Path, flag: &str, branch: &str, start_point: &str) -> bool {
    let mut args = vec!["checkout"];
    if !flag.is_empty() {
        args.push(flag);
    }
    args.push(branch);
    if flag == "-B" {
        args.push(start_point);
    }
    crate::git::run(project, &args, 120)
        .map(|o| o.status == 0)
        .unwrap_or(false)
}

pub fn rebase_onto(project: &Path, onto: &str) -> bool {
    crate::git::run(project, &["rebase", onto], 300)
        .map(|o| o.status == 0)
        .unwrap_or(false)
}

/// A conflicted rebase leaves `.git/rebase-merge` behind. Left alone, every
/// later task fails immediately with git's own "a rebase is already in
/// progress" — one real conflict wedges every task after it, not just the one
/// that hit it.
pub fn rebase_abort(project: &Path) {
    let _ = crate::git::run(project, &["rebase", "--abort"], 60);
}

pub fn merge_ff(project: &Path, branch: &str) -> bool {
    crate::git::run(project, &["merge", "--ff-only", branch], 120)
        .map(|o| o.status == 0)
        .unwrap_or(false)
}

pub fn push_head(project: &Path) -> bool {
    crate::git::run(project, &["push", "-q", "origin", "HEAD"], 300)
        .map(|o| o.status == 0)
        .unwrap_or(false)
}

pub fn delete_branch(project: &Path, branch: &str) {
    let _ = crate::git::run(project, &["branch", "-D", branch], 60);
}

pub fn stage_all(project: &Path) -> bool {
    crate::git::run(project, &["add", "-A", "--", ":(exclude).autopilot"], 120)
        .map(|o| o.status == 0)
        .unwrap_or(false)
}

pub fn has_staged_changes(project: &Path) -> bool {
    crate::git::run(project, &["diff", "--cached", "--quiet"], 60)
        .map(|o| o.status != 0)
        .unwrap_or(false)
}

pub fn commit(project: &Path, message: &str) -> bool {
    crate::git::run(project, &["commit", "-q", "-m", message], 120)
        .map(|o| o.status == 0)
        .unwrap_or(false)
}

pub fn commit_if_dirty(project: &Path, title: &str, issue: u64, main_branch: &str) -> bool {
    if !guard_not_main(project, main_branch) {
        return false;
    }
    if !stage_all(project) {
        return false;
    }
    if !has_staged_changes(project) {
        crate::log::info("the agent committed its own work; nothing left to stage");
        return true;
    }
    let message = format!(
        "{title}\n\nImplements the task described in issue #{issue}, which comes from the approved\nplan. The project's verification commands passed before this commit was made.\n\nCloses #{issue}"
    );
    commit(project, &message)
}

pub fn commit_wip(project: &Path, issue: u64, main_branch: &str, work_branch: &str) -> bool {
    if !guard_not_main(project, main_branch) {
        return false;
    }
    if !stage_all(project) {
        return false;
    }
    if !has_staged_changes(project) {
        return true;
    }
    let message = format!(
        "WIP: unverified work for issue #{issue}\n\nPaused for a coordinator. This commit has NOT passed the project's\nverification commands and must not be merged to {work_branch}."
    );
    commit(project, &message)
}
