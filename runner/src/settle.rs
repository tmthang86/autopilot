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

pub fn checkout_task_branch(project: &Path, issue: u64) -> bool {
    let branch = format!("autopilot/task-{issue}");
    checkout(project, "-B", &branch)
}

pub fn checkout(project: &Path, flag: &str, branch: &str) -> bool {
    let mut args = vec!["checkout"];
    if !flag.is_empty() {
        args.push(flag);
    }
    args.push(branch);
    if flag == "-B" {
        args.push("autopilot/main");
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

pub fn commit_if_dirty(project: &Path, title: &str, issue: u64) -> bool {
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

pub fn commit_wip(project: &Path, issue: u64) -> bool {
    if !stage_all(project) {
        return false;
    }
    if !has_staged_changes(project) {
        return true;
    }
    let message = format!(
        "WIP: unverified work for issue #{issue}\n\nPaused for a coordinator. This commit has NOT passed the project's\nverification commands and must not be merged to autopilot/main."
    );
    commit(project, &message)
}

pub fn comment(repo: &str, issue: u64, body: &str) -> bool {
    crate::gh::run(
        &[
            "issue",
            "comment",
            &issue.to_string(),
            "--repo",
            repo,
            "--body",
            body,
        ],
        crate::gh::TIMEOUT_S,
    )
    .map(|o| o.ok())
    .unwrap_or(false)
}

pub fn add_label(repo: &str, issue: u64, label: &str) -> bool {
    crate::gh::run(
        &[
            "issue",
            "edit",
            &issue.to_string(),
            "--repo",
            repo,
            "--add-label",
            label,
        ],
        crate::gh::TIMEOUT_S,
    )
    .map(|o| o.ok())
    .unwrap_or(false)
}

pub fn close(repo: &str, issue: u64) -> bool {
    crate::gh::run(
        &["issue", "close", &issue.to_string(), "--repo", repo],
        crate::gh::TIMEOUT_S,
    )
    .map(|o| o.ok())
    .unwrap_or(false)
}
