//! Regression tests for the branch-safety fixes: the accumulation branch is
//! created from scratch on a fresh project (it previously never was, and only
//! test fixtures pre-creating it masked the gap), the guard actually refuses
//! a write on the real main branch (it previously existed but was never
//! called), and a conflicted rebase is aborted rather than left to wedge
//! every later task.

mod common;

use autopilot::settle;
use common::make_repo;
use std::path::Path;
use std::process::Command;

fn git(dir: &Path, args: &[&str]) -> std::process::Output {
    Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .expect("run git")
}

fn current_branch(dir: &Path) -> String {
    String::from_utf8_lossy(&git(dir, &["rev-parse", "--abbrev-ref", "HEAD"]).stdout)
        .trim()
        .to_string()
}

#[test]
fn start_task_branch_creates_the_accumulation_branch_from_scratch() {
    // make_repo() only ever creates `main` — no install step creates
    // `autopilot/main`, so the very first task on a fresh project must reach
    // this from nothing.
    let repo = make_repo();
    assert!(
        !git(
            &repo,
            &["rev-parse", "--verify", "--quiet", "autopilot/main"]
        )
        .status
        .success(),
        "the fixture must not already carry the accumulation branch"
    );

    assert!(settle::start_task_branch(
        &repo,
        7,
        "main",
        "autopilot/main"
    ));

    assert!(
        git(
            &repo,
            &["rev-parse", "--verify", "--quiet", "autopilot/main"]
        )
        .status
        .success(),
        "the accumulation branch must now exist"
    );
    assert_eq!(
        current_branch(&repo),
        "autopilot/task-7",
        "the task branch is checked out, cut from the newly created accumulation branch"
    );
}

#[test]
fn start_task_branch_is_idempotent_when_the_accumulation_branch_already_exists() {
    let repo = make_repo();
    assert!(settle::start_task_branch(
        &repo,
        1,
        "main",
        "autopilot/main"
    ));
    git(&repo, &["checkout", "-q", "autopilot/main"]);
    // A second task must reuse the existing accumulation branch, not fail or
    // recreate it from `main` and lose whatever the first task landed on it.
    assert!(settle::start_task_branch(
        &repo,
        2,
        "main",
        "autopilot/main"
    ));
    assert_eq!(current_branch(&repo), "autopilot/task-2");
}

#[test]
fn commit_wip_refuses_on_the_main_branch() {
    let repo = make_repo();
    assert_eq!(current_branch(&repo), "main");
    std::fs::write(repo.join("dirty.txt"), "uncommitted").expect("write");
    let before = git(&repo, &["rev-parse", "HEAD"]).stdout;

    assert!(
        !settle::commit_wip(&repo, 99, "main", "autopilot/main"),
        "must refuse to write while HEAD is on the project's real main branch"
    );

    let after = git(&repo, &["rev-parse", "HEAD"]).stdout;
    assert_eq!(before, after, "no commit may land on main");
}

#[test]
fn commit_if_dirty_refuses_on_the_main_branch() {
    let repo = make_repo();
    std::fs::write(repo.join("dirty.txt"), "uncommitted").expect("write");
    let before = git(&repo, &["rev-parse", "HEAD"]).stdout;

    assert!(!settle::commit_if_dirty(&repo, "a title", 99, "main"));

    let after = git(&repo, &["rev-parse", "HEAD"]).stdout;
    assert_eq!(before, after, "no commit may land on main");
}

#[test]
fn rebase_abort_clears_a_conflicted_rebase() {
    let repo = make_repo();
    std::fs::write(repo.join("seed.txt"), "from main").expect("write");
    git(&repo, &["commit", "-aqm", "change on main"]);
    git(&repo, &["checkout", "-qb", "autopilot/task-1"]);
    // One commit behind main's tip, conflicting on the same line.
    git(&repo, &["reset", "-q", "--hard", "HEAD^"]);
    std::fs::write(repo.join("seed.txt"), "from the task branch").expect("write");
    git(&repo, &["commit", "-aqm", "conflicting change"]);

    let rebase = git(&repo, &["rebase", "main"]);
    assert!(
        !rebase.status.success(),
        "the rebase must actually conflict"
    );
    assert!(
        repo.join(".git/rebase-merge").exists(),
        "git must be left mid-rebase for this test to mean anything"
    );

    settle::rebase_abort(&repo);

    assert!(
        !repo.join(".git/rebase-merge").exists(),
        "rebase_abort must clear the interrupted rebase"
    );
    // A wedged rebase makes git refuse to start a new one at all; this is the
    // concrete symptom every later task would have hit.
    let retry = git(&repo, &["rebase", "main"]);
    assert!(
        retry.status.success() || !String::from_utf8_lossy(&retry.stderr).contains("rebase-merge"),
        "a later rebase must not fail with 'already a rebase-merge directory'"
    );
}
