//! The tail of the pipeline: the final review lenses, the rebase, the final
//! verify, and the fast-forward merge. Split out so `pipeline.rs` stays small.

use std::path::Path;
use std::time::Instant;

use crate::config::Config;
use crate::journal::Journal;
use crate::pipeline::{Outcome, Task};
use crate::pipeline_settle::{fail, pause};
use crate::pipeline_steps::{run_reviewer, Step};
use crate::queue::Queue;
use crate::state::State;

#[allow(clippy::too_many_arguments)]
pub fn finish(
    cfg: &Config,
    project: &Path,
    queue: &Queue,
    task: &Task,
    journal: &mut Journal,
    state: &mut State,
    started: &Instant,
    wake: &str,
    attempt: u32,
    round: u32,
    review_tier: &str,
    task_branch: &str,
    start_sha: &str,
) -> Outcome {
    for lens in &cfg.pipeline.review_lenses {
        if budget_exhausted(cfg, started) {
            return pause(project, queue, task.issue, "wake budget exhausted");
        }
        match run_reviewer(
            cfg,
            project,
            queue,
            task,
            task_branch,
            wake,
            attempt,
            round,
            review_tier,
            Some(lens),
            journal,
        ) {
            Step::Ok => {}
            Step::Unavailable => return Outcome::Unavailable,
            Step::Failed(detail) => {
                return fail(
                    cfg,
                    state,
                    project,
                    queue,
                    task,
                    start_sha,
                    task_branch,
                    "review",
                    &detail,
                );
            }
        }
        let dir =
            crate::verdict::dir_for(project, task.issue, wake, attempt, &format!("lens-{lens}"));
        let verdict = match crate::verdict::read(&crate::verdict::verdict_path(&dir, "reviewer")) {
            Ok(v) => v,
            Err(e) => {
                return fail(
                    cfg,
                    state,
                    project,
                    queue,
                    task,
                    start_sha,
                    task_branch,
                    "verdict",
                    &e.to_string(),
                );
            }
        };
        if !verdict.is_pass() {
            return pause(
                project,
                queue,
                task.issue,
                &format!("final review lens {lens} rejected: {}", verdict.reason),
            );
        }
    }

    if !crate::settle::rebase_onto(project, "autopilot/main") {
        return pause(
            project,
            queue,
            task.issue,
            "rebase conflict onto autopilot/main",
        );
    }
    if let Err(f) = crate::verify::run(cfg, project) {
        return fail(
            cfg,
            state,
            project,
            queue,
            task,
            start_sha,
            task_branch,
            "verify",
            &f.output,
        );
    }
    if !crate::settle::commit_if_dirty(project, task.title, task.issue) {
        return fail(
            cfg,
            state,
            project,
            queue,
            task,
            start_sha,
            task_branch,
            "commit",
            "staging failed",
        );
    }
    if !crate::settle::checkout(project, "", "autopilot/main") {
        return pause(
            project,
            queue,
            task.issue,
            "could not check out the accumulation branch",
        );
    }
    if !crate::settle::merge_ff(project, task_branch) {
        return pause(
            project,
            queue,
            task.issue,
            "fast-forward onto the accumulation branch failed",
        );
    }
    if !crate::settle::push_head(project) {
        let _ = queue.release(task.issue);
        crate::settle::comment(
            &queue.repo,
            task.issue,
            "Implementation finished and verification passed, but pushing to the remote failed. The work exists only on this machine. Leaving this issue open.",
        );
        return Outcome::Failed;
    }

    let prepare_only = queue.has_label(task.issue, &cfg.autonomy.prepare_only_label);
    let _ = queue.release(task.issue);
    if prepare_only {
        crate::settle::add_label(&queue.repo, task.issue, &cfg.autonomy.prepare_only_label);
        crate::settle::comment(
            &queue.repo,
            task.issue,
            "Implementation complete and the automated checks passed. This task's correctness depends on behaviour a person has to observe, so it stays open for human acceptance.",
        );
    } else {
        crate::settle::comment(
            &queue.repo,
            task.issue,
            "Completed automatically. Verification passed and the work is committed.",
        );
        if !crate::settle::close(&queue.repo, task.issue) {
            crate::log::error(&format!(
                "#{} could NOT be closed — its work is verified and pushed; close it by hand",
                task.issue
            ));
        }
    }

    crate::settle::delete_branch(project, task_branch);
    crate::log::info(&format!("#{} merged", task.issue));
    Outcome::Merged
}

fn budget_exhausted(cfg: &Config, started: &Instant) -> bool {
    started.elapsed().as_secs() + cfg.pipeline.turn_timeout_s > cfg.pipeline.wake_timeout_s
}
