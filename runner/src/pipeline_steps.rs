//! The small per-role steps the pipeline composes: run the implementer, run the
//! tester and read its verdict, and run a reviewer (adjudication or lens).

use std::path::Path;

use crate::config::Config;
use crate::harness::Classification;
use crate::journal::Journal;
use crate::pipeline::Task;
use crate::queue::Queue;

pub enum Step {
    Ok,
    Unavailable,
    Failed(String),
}

pub enum Tester {
    Pass(crate::verdict::Verdict),
    Reject(crate::verdict::Verdict),
    Unavailable,
    Failed(String),
}

pub fn run_implementer(
    cfg: &Config,
    project: &Path,
    task: &Task,
    task_branch: &str,
    journal: &mut Journal,
    round: u32,
) -> Step {
    let p = crate::role::prompt(
        "implement",
        cfg,
        task.issue,
        task.title,
        task.body,
        task_branch,
        &task.intent,
        None,
        None,
    );
    match crate::role::run_role(
        cfg,
        project,
        "implement",
        task.tier,
        round,
        None,
        &p,
        journal,
    ) {
        Ok(r) if r.classify == Classification::ProviderUnavailable => Step::Unavailable,
        Ok(r) if r.classify == Classification::TaskFailure => {
            Step::Failed("the implementer role failed".into())
        }
        Err(e) => Step::Failed(e),
        Ok(_) => Step::Ok,
    }
}

#[allow(clippy::too_many_arguments)]
pub fn run_tester(
    cfg: &Config,
    project: &Path,
    queue: &Queue,
    task: &Task,
    task_branch: &str,
    wake: &str,
    attempt: u32,
    round: u32,
    test_tier: &str,
    journal: &mut Journal,
) -> Tester {
    let kind = format!("round-{round}");
    let dir = match crate::verdict::prepare(project, task.issue, wake, attempt, &kind, "tester") {
        Ok(d) => d,
        Err(e) => return Tester::Failed(e),
    };
    let tester_path = crate::verdict::verdict_path(&dir, "tester");
    let p = crate::role::prompt(
        "test",
        cfg,
        task.issue,
        task.title,
        task.body,
        task_branch,
        &task.intent,
        Some(&tester_path),
        None,
    );
    match crate::role::run_role(cfg, project, "test", test_tier, round, None, &p, journal) {
        Ok(r) if r.classify == Classification::ProviderUnavailable => {
            let _ = queue.release(task.issue);
            Tester::Unavailable
        }
        Ok(r) if r.classify == Classification::TaskFailure => {
            Tester::Failed("the tester role failed".into())
        }
        Err(e) => Tester::Failed(e),
        Ok(_) => match crate::verdict::read(&tester_path) {
            Ok(v) if v.is_pass() => Tester::Pass(v),
            Ok(v) => Tester::Reject(v),
            Err(e) => Tester::Failed(e.to_string()),
        },
    }
}

#[allow(clippy::too_many_arguments)]
pub fn run_reviewer(
    cfg: &Config,
    project: &Path,
    queue: &Queue,
    task: &Task,
    task_branch: &str,
    wake: &str,
    attempt: u32,
    round: u32,
    review_tier: &str,
    lens: Option<&str>,
    journal: &mut Journal,
) -> Step {
    let kind = match lens {
        Some(l) => format!("lens-{l}"),
        None => format!("round-{round}"),
    };
    let dir = match crate::verdict::prepare(project, task.issue, wake, attempt, &kind, "reviewer") {
        Ok(d) => d,
        Err(e) => return Step::Failed(e),
    };
    let reviewer_path = crate::verdict::verdict_path(&dir, "reviewer");
    let p = crate::role::prompt(
        "review",
        cfg,
        task.issue,
        task.title,
        task.body,
        task_branch,
        &task.intent,
        Some(&reviewer_path),
        lens,
    );
    match crate::role::run_role(
        cfg,
        project,
        "review",
        review_tier,
        round,
        lens,
        &p,
        journal,
    ) {
        Ok(r) if r.classify == Classification::ProviderUnavailable => {
            let _ = queue.release(task.issue);
            Step::Unavailable
        }
        Ok(r) if r.classify == Classification::TaskFailure => {
            Step::Failed(format!("lens {} failed", lens.unwrap_or("adjudicate")))
        }
        Err(e) => Step::Failed(e),
        Ok(_) => Step::Ok,
    }
}
