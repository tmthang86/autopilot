//! The three-role pipeline: implement → verify → test → review, bounded rounds,
//! final review lenses that always run, a rebase, and a fast-forward merge.
use crate::config::Config;
use crate::journal::{Event, Journal};
use crate::pipeline_implement::implement_or_continue;
use crate::pipeline_settle::{fail, pause};
use crate::pipeline_steps::{run_reviewer, run_tester, Step, Tester};
use crate::queue::Queue;
use crate::state::State;
use std::path::{Path, PathBuf};
use std::time::Instant;
pub enum Outcome {
    Merged,
    Failed,
    Paused,
    Unavailable,
}

pub struct Task<'a> {
    pub issue: u64,
    pub title: &'a str,
    pub body: &'a str,
    pub tier: &'a str,
    pub intent: Vec<PathBuf>,
}

/// Checked immediately before a role starts (decision 33): a role cut off
/// mid-run forges an orphaned journal entry.
macro_rules! budget_gate {
    ($cfg:ident, $project:ident, $queue:ident, $issue:expr, $started:ident, $before:literal) => {
        if let Some(o) = crate::pipeline_implement::check_budget(
            $cfg, $project, $queue, $issue, &$started, $before,
        ) {
            return o;
        }
    };
}

pub fn run(
    cfg: &Config,
    project: &Path,
    queue: &Queue,
    task: &Task,
    journal: &mut Journal,
    state: &mut State,
) -> Outcome {
    let started = Instant::now();
    let wake = journal.wake().to_string();
    let attempt = state.attempt_count(task.issue);
    let task_branch = format!("autopilot/task-{}", task.issue);

    let (review_tier, test_tier, start_sha) =
        match crate::pipeline_implement::setup(cfg, project, queue, task) {
            Ok(v) => v,
            Err(o) => return o,
        };
    if let Some(outcome) = implement_or_continue(
        cfg,
        state,
        project,
        queue,
        task,
        &task_branch,
        journal,
        0,
        &start_sha,
        &started,
    ) {
        return outcome;
    }

    let mut round = 0u32;
    loop {
        if crate::pipeline_implement::budget_exhausted(cfg, &started) {
            return pause(cfg, project, queue, task.issue, "wake budget exhausted");
        }
        if let Err(f) = crate::verify::run(cfg, project) {
            crate::log::warn(&format!(
                "verify red at {}; returning to the implementer",
                f.name
            ));
            if let Some(outcome) = implement_or_continue(
                cfg,
                state,
                project,
                queue,
                task,
                &task_branch,
                journal,
                round,
                &start_sha,
                &started,
            ) {
                return outcome;
            }
            continue;
        }

        budget_gate!(cfg, project, queue, task.issue, started, "the tester");
        match run_tester(
            cfg,
            project,
            queue,
            task,
            &task_branch,
            &wake,
            attempt,
            round,
            &test_tier,
            journal,
        ) {
            Tester::Pass(v) => {
                journal
                    .append(Event::Verdict {
                        role: "test".into(),
                        round,
                        verdict: v.verdict,
                        reason: v.reason,
                    })
                    .ok();
                break;
            }
            Tester::Reject(v) => {
                round += 1;
                if round > cfg.pipeline.max_rounds {
                    return pause(cfg, project, queue, task.issue, "rounds exhausted");
                }
                journal
                    .append(Event::Verdict {
                        role: "test".into(),
                        round: round - 1,
                        verdict: v.verdict,
                        reason: v.reason,
                    })
                    .ok();
                budget_gate!(cfg, project, queue, task.issue, started, "the reviewer");
                match run_reviewer(
                    cfg,
                    project,
                    queue,
                    task,
                    &task_branch,
                    &wake,
                    attempt,
                    round,
                    &review_tier,
                    None,
                    journal,
                ) {
                    Step::Ok => continue,
                    Step::Unavailable => return Outcome::Unavailable,
                    Step::Failed(detail) => {
                        return fail(
                            cfg,
                            state,
                            project,
                            queue,
                            task,
                            &start_sha,
                            &task_branch,
                            "review",
                            &detail,
                        );
                    }
                }
            }
            Tester::Unavailable => return Outcome::Unavailable,
            Tester::Failed(detail) => {
                return fail(
                    cfg,
                    state,
                    project,
                    queue,
                    task,
                    &start_sha,
                    &task_branch,
                    "verdict",
                    &detail,
                );
            }
        }
    }

    crate::pipeline_land::finish(
        cfg,
        project,
        queue,
        task,
        journal,
        state,
        &started,
        &wake,
        attempt,
        round,
        &review_tier,
        &task_branch,
        &start_sha,
    )
}
