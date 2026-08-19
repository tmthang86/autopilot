//! The implement step and the wake-budget check, shared by the initial pass and
//! the red-verify pass that returns to the implementer.

use std::path::Path;
use std::time::Instant;

use crate::config::Config;
use crate::journal::Journal;
use crate::pipeline::{Outcome, Task};
use crate::pipeline_settle::{fail, pause};
use crate::pipeline_steps::{run_implementer, Step};
use crate::queue::Queue;
use crate::state::State;

pub fn budget_exhausted(cfg: &Config, started: &Instant) -> bool {
    started.elapsed().as_secs() + cfg.pipeline.turn_timeout_s > cfg.pipeline.wake_timeout_s
}

/// Checked immediately before a role starts, not only once per round: `verify`
/// runs between rounds and can itself take most of the remaining wake, leaving
/// an earlier check stale. A role cut off mid-run forges an orphaned journal
/// entry, which is what this check exists to prevent (decision 33).
pub fn check_budget(
    cfg: &Config,
    project: &Path,
    queue: &Queue,
    issue: u64,
    started: &Instant,
    before: &str,
) -> Option<Outcome> {
    if budget_exhausted(cfg, started) {
        Some(pause(
            cfg,
            project,
            queue,
            issue,
            &format!("wake budget exhausted before {before}"),
        ))
    } else {
        None
    }
}

/// Resolves the test/review tiers, resets the tree, and checks out a fresh
/// task branch — the pipeline's setup, before any role runs.
pub fn setup(
    cfg: &Config,
    project: &Path,
    queue: &Queue,
    task: &Task,
) -> Result<(String, String, String), Outcome> {
    let review_tier = crate::tier::resolve(cfg, task.tier, cfg.roles.review.tier_offset)
        .map(|(n, _)| n.to_string())
        .map_err(|_| {
            pause(
                cfg,
                project,
                queue,
                task.issue,
                "review tier cannot be resolved",
            )
        })?;
    let test_tier = crate::tier::resolve(cfg, task.tier, cfg.roles.test.tier_offset)
        .map(|(n, _)| n.to_string())
        .map_err(|_| {
            pause(
                cfg,
                project,
                queue,
                task.issue,
                "test tier cannot be resolved",
            )
        })?;
    crate::settle::reset(project, "HEAD");
    if !crate::settle::start_task_branch(
        project,
        task.issue,
        &cfg.project.main_branch,
        &cfg.project.work_branch,
    ) {
        return Err(pause(
            cfg,
            project,
            queue,
            task.issue,
            "could not create the task branch",
        ));
    }
    Ok((review_tier, test_tier, crate::settle::start_sha(project)))
}

/// Run the implementer unless the wake budget is already gone. Returns `Some`
/// when the pipeline must stop now, and `None` to continue to the next step.
#[allow(clippy::too_many_arguments)]
pub fn implement_or_continue(
    cfg: &Config,
    state: &mut State,
    project: &Path,
    queue: &Queue,
    task: &Task,
    task_branch: &str,
    journal: &mut Journal,
    round: u32,
    start_sha: &str,
    started: &Instant,
) -> Option<Outcome> {
    if budget_exhausted(cfg, started) {
        return Some(pause(
            cfg,
            project,
            queue,
            task.issue,
            "wake budget exhausted before the implementer",
        ));
    }
    match run_implementer(cfg, project, task, task_branch, journal, round) {
        Step::Ok => None,
        Step::Unavailable => {
            let _ = queue.release(task.issue);
            Some(Outcome::Unavailable)
        }
        Step::Failed(detail) => Some(fail(
            cfg,
            state,
            project,
            queue,
            task,
            start_sha,
            task_branch,
            "implement",
            &detail,
        )),
    }
}
