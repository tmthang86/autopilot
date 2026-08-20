//! The run-once flow: guard, pick, resolve intent and tier, claim, and run the
//! pipeline. Exit 0 means "ran, or correctly declined to run".
use std::path::Path;
use std::process::ExitCode;

use crate::config::Config;
use crate::guard::GuardOutcome;
use crate::journal::Event;
use crate::pipeline;
use crate::queue::Queue;
use crate::state::State;

pub fn run(project: &Path) -> ExitCode {
    let ap = project.join(".autopilot");
    if std::fs::create_dir_all(ap.join("logs")).is_err() {
        return ExitCode::from(1);
    }
    crate::log::set_file(Some(
        ap.join(format!("logs/{}.log", crate::log::date_today())),
    ));

    let cfg = match Config::load(project) {
        Ok(c) => c,
        Err(e) => {
            crate::log::error(&e.to_string());
            return ExitCode::from(1);
        }
    };
    let mut state = match State::init(&ap.join("state.json")) {
        Ok(s) => s,
        Err(e) => {
            crate::log::error(&e.to_string());
            return ExitCode::from(1);
        }
    };
    let mut journal = match crate::journal::Journal::open(project, cfg.journal.max_bytes) {
        Ok(j) => j,
        Err(e) => {
            crate::log::error(&e.to_string());
            return ExitCode::from(1);
        }
    };
    let mut queue = match Queue::init(project) {
        Ok(q) => q,
        Err(e) => {
            crate::log::error(&e.to_string());
            return ExitCode::from(1);
        }
    };
    let _lock = match crate::guard::all(&cfg, &mut state, project) {
        Ok(lock) => lock,
        Err(GuardOutcome::StandDown(reason)) => {
            crate::log::info(&reason);
            stood_down(&mut journal, None, None);
            return ExitCode::SUCCESS;
        }
    };

    queue.load(&cfg);
    state.prune_attempts(&queue.open_issue_numbers());
    let Some(issue) = queue.pick(&cfg) else {
        crate::log::info("queue is empty");
        stood_down(&mut journal, None, None);
        return ExitCode::SUCCESS;
    };

    let title = queue.field(issue, "title").unwrap_or_default();
    let body = queue.field(issue, "body").unwrap_or_default();
    crate::log::info(&format!("starting #{issue}: {title}"));

    let intent = match crate::intent::resolve(&body, project, &cfg.queue.intent_marker) {
        Ok(v) => v,
        Err(e) => {
            crate::log::error(&e.to_string());
            crate::settle::reset(project, "HEAD");
            crate::gh::comment(
                &queue.repo,
                issue,
                &format!(
                    "This task was refused before any work started: it does not name the documents that authorise it. Add an '{}' line listing at least one existing file, then remove the 'blocked' label.",
                    cfg.queue.intent_marker
                ),
            );
            crate::gh::add_label(&queue.repo, issue, "blocked");
            stood_down(&mut journal, Some(issue), None);
            return ExitCode::SUCCESS;
        }
    };

    let tier = queue
        .labels(issue)
        .iter()
        .find_map(|l| {
            l.strip_prefix(&cfg.queue.tier_label_prefix)
                .map(str::to_string)
        })
        .unwrap_or_else(|| cfg.agent.default_tier.clone());

    let mut needed = vec![tier.clone()];
    if let Ok((n, _)) = crate::tier::resolve(&cfg, &tier, cfg.roles.test.tier_offset) {
        needed.push(n.to_string());
    }
    if let Ok((n, _)) = crate::tier::resolve(&cfg, &tier, cfg.roles.review.tier_offset) {
        needed.push(n.to_string());
    }
    needed.dedup();
    let refs: Vec<&str> = needed.iter().map(String::as_str).collect();
    if let Err(GuardOutcome::StandDown(reason)) = crate::guard::tiers(&cfg, &refs) {
        crate::log::info(&reason);
        stood_down(&mut journal, Some(issue), Some(tier.clone()));
        let _ = journal.append(Event::WakeEnd {
            outcome: "provider_unavailable".into(),
        });
        backoff(&mut state);
        return ExitCode::SUCCESS;
    }

    if let Err(e) = queue.claim(issue) {
        crate::log::error(&format!(
            "not running the agent for #{issue}: the issue was never claimed: {e}"
        ));
        let _ = journal.append(Event::WakeStart {
            issue: Some(issue),
            tier: Some(tier.clone()),
        });
        let _ = journal.append(Event::WakeEnd {
            outcome: "failed".into(),
        });
        return ExitCode::from(1);
    }
    state.bump("tasks_today");
    state.save().ok();
    let _ = journal.append(Event::WakeStart {
        issue: Some(issue),
        tier: Some(tier.clone()),
    });

    let task = pipeline::Task {
        issue,
        title: &title,
        body: &body,
        tier: &tier,
        intent,
    };
    let outcome = pipeline::run(&cfg, project, &queue, &task, &mut journal, &mut state);
    match outcome {
        pipeline::Outcome::Merged => {
            let _ = journal.append(Event::WakeEnd {
                outcome: "merged".into(),
            });
            state.set_num("consecutive_failures", 0);
            state.set_num("backoff_step", 0);
        }
        pipeline::Outcome::Failed => {
            let _ = journal.append(Event::WakeEnd {
                outcome: "failed".into(),
            });
            state.bump("consecutive_failures");
        }
        pipeline::Outcome::Paused => {
            let _ = journal.append(Event::WakeEnd {
                outcome: "paused".into(),
            });
        }
        pipeline::Outcome::Unavailable => {
            let _ = journal.append(Event::WakeEnd {
                outcome: "provider_unavailable".into(),
            });
            backoff(&mut state);
            // Never attempted: an outage must not consume the day tasks.
            state.set_num("tasks_today", (state.get_num("tasks_today", 0) - 1).max(0));
        }
    }

    if state.get_num("consecutive_failures", 0) >= cfg.pacing.circuit_breaker_failures {
        let _ = std::fs::write(ap.join("STOP"), "");
        crate::log::error(&format!(
            "circuit breaker tripped after {} failures — STOP file created",
            state.get_num("consecutive_failures", 0)
        ));
    }
    state.save().ok();
    ExitCode::SUCCESS
}

fn stood_down(journal: &mut crate::journal::Journal, issue: Option<u64>, tier: Option<String>) {
    let _ = journal.append(Event::WakeStart { issue, tier });
    let _ = journal.append(Event::WakeEnd {
        outcome: "stood_down".into(),
    });
}
fn backoff(state: &mut State) {
    let step = state.get_num("backoff_step", 0);
    let seconds = (900 * (step + 1)).min(3600);
    state.set_num("resume_after", crate::log::unix_now() as i64 + seconds);
    state.bump("backoff_step");
    state.save().ok();
    crate::log::info(&format!("usage window closed; retrying in {seconds}s"));
}
