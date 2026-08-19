//! The claude adapter, carrying the two refinements paid for in
//! `observed-behaviour.md` §4 and §5: only the final `result` event decides the
//! outcome, and `is_error` is compared against `false` explicitly.

use super::{Check, Classification, Harness, RunOutcome, RunParams};
use crate::journal::CostSource;
use crate::spawn::SpawnOutcome;
use std::io;
use std::path::Path;
use std::process::Command;
use std::time::Instant;

pub struct Claude;

impl Harness for Claude {
    fn name(&self) -> &'static str {
        "claude"
    }

    fn proven(&self) -> bool {
        true
    }

    fn check(&self, _model: Option<&str>) -> Check {
        let mut cmd = Command::new("claude");
        cmd.args(["--version"]);
        match crate::spawn::run_capture(&mut cmd, b"", 30) {
            Ok(SpawnOutcome::Exited(o)) if o.status.success() => Check {
                available: true,
                models: Vec::new(),
                error: None,
                proven: true,
            },
            Ok(_) => Check {
                available: false,
                models: Vec::new(),
                error: Some("claude is not installed or not runnable".into()),
                proven: true,
            },
            Err(e) => Check {
                available: false,
                models: Vec::new(),
                error: Some(e.to_string()),
                proven: true,
            },
        }
    }

    fn probe(&self, _model: Option<&str>) -> bool {
        let mut cmd = Command::new("claude");
        cmd.args([
            "-p",
            "ok",
            "--model",
            "sonnet",
            "--tools",
            "",
            "--no-session-persistence",
        ]);
        matches!(
            crate::spawn::run_capture(&mut cmd, b"", 120),
            Ok(SpawnOutcome::Exited(o)) if o.status.success()
        )
    }

    fn run(&self, prompt: &str, cwd: &Path, log: &Path, p: &RunParams) -> io::Result<RunOutcome> {
        let start = Instant::now();
        let mut cmd = Command::new("claude");
        cmd.current_dir(cwd);
        cmd.args([
            "-p",
            "--add-dir",
            ".",
            "--output-format",
            "stream-json",
            "--verbose",
        ]);
        if p.permission_mode == "bypassPermissions" {
            cmd.arg("--dangerously-skip-permissions");
        } else {
            cmd.args(["--permission-mode", p.permission_mode]);
        }
        cmd.args([
            "--model",
            p.model,
            "--effort",
            p.effort,
            "--max-budget-usd",
            &format!("{}", p.budget_usd),
        ]);
        let outcome = crate::spawn::run_to_file(&mut cmd, prompt.as_bytes(), log, p.timeout_s)?;
        Ok(RunOutcome {
            status: outcome.status_code(),
            log: log.to_path_buf(),
            cost_usd: cost_from_log(log),
            cost_source: CostSource::Reported,
            duration_s: start.elapsed().as_secs(),
            model: p.model.to_string(),
        })
    }

    fn classify(&self, out: &RunOutcome) -> Classification {
        classify(out)
    }
}

fn cost_from_log(log: &Path) -> Option<f64> {
    let body = std::fs::read_to_string(log).ok()?;
    body.lines()
        .filter(|l| l.contains("\"type\":\"result\""))
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .filter_map(|v| v.get("total_cost_usd").and_then(|c| c.as_f64()))
        .next_back()
}

fn classify(out: &RunOutcome) -> Classification {
    if !out.log.exists() {
        return if out.status == Some(0) {
            Classification::Ok
        } else {
            Classification::TaskFailure
        };
    }
    let body = match std::fs::read_to_string(&out.log) {
        Ok(b) => b,
        Err(_) => {
            return if out.status == Some(0) {
                Classification::Ok
            } else {
                Classification::TaskFailure
            }
        }
    };

    for line in body.lines() {
        if line.contains("\"type\":\"rate_limit_event\"")
            && !line.contains("\"status\":\"allowed\"")
        {
            return Classification::ProviderUnavailable;
        }
    }

    let final_result = body.lines().rfind(|l| l.contains("\"type\":\"result\""));
    if let Some(line) = final_result {
        let is_error = serde_json::from_str::<serde_json::Value>(line)
            .ok()
            .and_then(|v| v.get("is_error").and_then(|e| e.as_bool()))
            .unwrap_or(true);
        if out.status == Some(0) && !is_error {
            return Classification::Ok;
        }
    } else if out.status == Some(0) {
        return Classification::Ok;
    }

    if Claude.probe(None) {
        Classification::TaskFailure
    } else {
        Classification::ProviderUnavailable
    }
}
