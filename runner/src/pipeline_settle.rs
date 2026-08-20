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
    // the WIP commit is a record for a person to recover. The runner itself
    // does NOT resume from it: a re-queued task restarts from the accumulation
    // branch (see open-items, "paused tasks restart from the accumulation
    // branch").
    let status = if pushed {
        "The work is preserved in a WIP commit on this machine's task branch and was pushed to the remote, so it survives losing the machine. The runner will NOT resume it automatically: recovering the work is a person's job, and a re-queued task restarts from the accumulation branch."
    } else if committed {
        crate::log::error(&format!(
            "#{issue} paused but the WIP commit could not be pushed — the work exists only on this machine"
        ));
        "The work is preserved in a local WIP commit but could NOT be pushed; it exists only on this machine. The runner will NOT resume it automatically."
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
