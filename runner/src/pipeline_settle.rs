//! The pipeline's settlement paths: pause (keep the work), fail (rewind it),
//! and release (the claim label). Split from `pipeline.rs` so both stay small.

use std::path::Path;

use crate::config::Config;
use crate::pipeline::{Outcome, Task};
use crate::queue::Queue;
use crate::state::State;

pub fn pause(project: &Path, queue: &Queue, issue: u64, reason: &str) -> Outcome {
    crate::settle::commit_wip(project, issue);
    crate::settle::push_head(project);
    crate::settle::comment(
        &queue.repo,
        issue,
        &format!("Blocked — this needs a decision:\n\n{reason}"),
    );
    crate::settle::add_label(&queue.repo, issue, "blocked");
    release(queue, issue);
    crate::log::info(&format!("#{issue} paused: {reason}"));
    Outcome::Paused
}

#[allow(clippy::too_many_arguments)]
pub fn fail(
    cfg: &Config,
    state: &mut State,
    project: &Path,
    queue: &Queue,
    task: &Task,
    start_sha: &str,
    task_branch: &str,
    reason: &str,
    detail: &str,
) -> Outcome {
    let n = state.record_attempt(task.issue);
    state.save().ok();
    if n >= cfg.pacing.max_attempts_per_issue {
        return pause(
            project,
            queue,
            task.issue,
            &format!(
                "{reason}; attempts exhausted ({n}/{}): {detail}",
                cfg.pacing.max_attempts_per_issue
            ),
        );
    }
    crate::settle::reset(project, start_sha);
    crate::settle::delete_branch(project, task_branch);
    crate::settle::comment(
        &queue.repo,
        task.issue,
        &format!(
            "Automated attempt failed at: {reason}\n\n```\n{detail}\n```\n\nThe working tree was reset; nothing was committed."
        ),
    );
    release(queue, task.issue);
    crate::log::info(&format!(
        "#{} failed at {} and was reset",
        task.issue, reason
    ));
    Outcome::Failed
}

pub fn release(queue: &Queue, issue: u64) {
    if queue.release(issue).is_err() {
        crate::log::error(&format!(
            "#{issue} still carries status:in-progress — an excluded label, so the queue will skip it until someone removes it"
        ));
    }
}
