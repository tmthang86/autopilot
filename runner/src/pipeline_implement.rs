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
