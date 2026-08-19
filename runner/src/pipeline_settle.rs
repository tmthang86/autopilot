//! The pipeline's settlement paths: pause (keep the work), fail (rewind it),
//! and release (the claim label). Split from `pipeline.rs` so both stay small.

use std::path::Path;

use crate::config::Config;
use crate::pipeline::{Outcome, Task};
use crate::queue::Queue;
use crate::state::State;

pub fn pause(cfg: &Config, project: &Path, queue: &Queue, issue: u64, reason: &str) -> Outcome {
    let committed = crate::settle::commit_wip(
        project,
        issue,
        &cfg.project.main_branch,
        &cfg.project.work_branch,
    );
    let pushed = committed && crate::settle::push_head(project);
    // Never claim "safely parked" when it might not be on disk or the remote —
    // a paused task is only resumable from what actually landed.
    let status = if pushed {
        "This commit was pushed to the remote."
    } else if committed {
        crate::log::error(&format!(
            "#{issue} paused but the WIP commit could not be pushed — the work exists only on this machine"
        ));
        "The work is committed locally but could NOT be pushed to the remote; it exists only on this machine."
    } else {
        crate::log::error(&format!(
            "#{issue} paused but nothing could be committed — no work was saved"
        ));
        "The attempted work could NOT be committed; nothing was saved."
    };
    crate::gh::comment(
        &queue.repo,
        issue,
        &format!("Blocked — this needs a decision:\n\n{reason}\n\n{status}"),
    );
    crate::gh::add_label(&queue.repo, issue, "blocked");
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
            cfg,
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
    crate::gh::comment(
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
