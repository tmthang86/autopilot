//! The pi adapter. Measured on 2026-08-19 (transcripts under tests/fixtures/):
//! `pi -p --mode json` exits 0 even when the model errors, so classification
//! reads `stopReason`/`errorMessage` from the stream, never the exit code.

use super::{Check, Classification, Harness, RunOutcome, RunParams};
use crate::journal::CostSource;
use crate::spawn::SpawnOutcome;
use std::io;
use std::path::Path;
use std::process::Command;
use std::time::Instant;

pub struct Pi;

impl Harness for Pi {
    fn name(&self) -> &'static str {
        "pi"
    }

    fn proven(&self) -> bool {
        true
    }

    fn check(&self, model: Option<&str>) -> Check {
        let provider = provider_of(model);
        let mut cmd = Command::new("pi");
        cmd.args(["auth", "check", "--provider", provider, "--json"]);
        match crate::spawn::run_capture(&mut cmd, b"", 30) {
            Ok(SpawnOutcome::Exited(o)) => {
                let stdout = String::from_utf8_lossy(&o.stdout);
                let v = serde_json::from_str::<serde_json::Value>(stdout.as_ref()).ok();
                let ready = v
                    .as_ref()
                    .and_then(|v| v.get("status"))
                    .and_then(|s| s.as_str())
                    == Some("ready");
                Check {
                    available: ready,
                    models: self.list_models(),
                    error: if ready {
                        None
                    } else {
                        Some(stdout.trim().to_string())
                    },
                    proven: true,
                }
            }
            _ => Check {
                available: false,
                models: Vec::new(),
                error: Some("pi is not installed or not runnable".into()),
                proven: true,
            },
        }
    }

    fn probe(&self, model: Option<&str>) -> bool {
        let provider = provider_of(model);
        let model = model.unwrap_or("deepseek/deepseek-v4-flash");
        let mut cmd = Command::new("pi");
        cmd.args([
            "-p",
            "--mode",
            "json",
            "--provider",
            provider,
            "--model",
            model,
            "--thinking",
            "off",
            "--no-session",
        ]);
        match crate::spawn::run_capture(&mut cmd, b"ok", 120) {
            Ok(SpawnOutcome::Exited(o)) => {
                o.status.success() && !stream_has_error(String::from_utf8_lossy(&o.stdout).as_ref())
            }
            _ => false,
        }
    }

    fn run(&self, prompt: &str, cwd: &Path, log: &Path, p: &RunParams) -> io::Result<RunOutcome> {
        let start = Instant::now();
        let provider = provider_of(Some(p.model));
        let mut cmd = Command::new("pi");
        cmd.current_dir(cwd);
        cmd.args([
            "-p",
            "--mode",
            "json",
            "--provider",
            provider,
            "--model",
            p.model,
            "--thinking",
            "off",
            "--no-session",
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
        if !out.log.exists() {
            return if out.status == Some(0) {
                Classification::Ok
            } else {
                Classification::TaskFailure
            };
        }
        let body = match std::fs::read_to_string(&out.log) {
            Ok(b) => b,
            Err(_) => return Classification::TaskFailure,
        };
        if out.status == Some(0) && !stream_has_error(&body) {
            return Classification::Ok;
        }
        if self.probe(Some(&out.model)) {
            Classification::TaskFailure
        } else {
            Classification::ProviderUnavailable
        }
    }
}

impl Pi {
    fn list_models(&self) -> Vec<String> {
        let mut cmd = Command::new("pi");
        cmd.args(["--list-models"]);
        match crate::spawn::run_capture(&mut cmd, b"", 60) {
            Ok(SpawnOutcome::Exited(o)) if o.status.success() => parse_models(&o.stdout),
            _ => Vec::new(),
        }
    }
}

fn provider_of(model: Option<&str>) -> &str {
    model
        .and_then(|m| m.split('/').next())
        .unwrap_or("deepseek")
}

fn stream_has_error(body: &str) -> bool {
    body.lines()
        .any(|l| l.contains("\"stopReason\":\"error\"") || l.contains("\"errorMessage\""))
}

fn parse_models(out: &[u8]) -> Vec<String> {
    let text = String::from_utf8_lossy(out);
    let mut models = Vec::new();
    for line in text.lines().skip(1) {
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() >= 2 {
            models.push(format!("{}/{}", cols[0], cols[1]));
        }
    }
    models
}

fn cost_from_log(log: &Path) -> Option<f64> {
    let body = std::fs::read_to_string(log).ok()?;
    body.lines()
        .filter(|l| l.contains("\"type\":\"message_end\""))
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .filter_map(|v| {
            v.pointer("/message/usage/cost/total")
                .and_then(|c| c.as_f64())
        })
        .next_back()
}
