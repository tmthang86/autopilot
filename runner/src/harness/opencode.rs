//! The opencode adapter. Unproven: it has never produced a real run transcript
//! on this machine, because `opencode models` fails on an invalid user config.
//! It reports that error verbatim rather than pretending the model list is
//! empty.

use super::{Check, Classification, Harness, RunOutcome, RunParams};
use crate::journal::CostSource;
use crate::spawn::SpawnOutcome;
use std::io;
use std::path::Path;
use std::process::Command;
use std::time::Instant;

pub struct Opencode;

impl Harness for Opencode {
    fn name(&self) -> &'static str {
        "opencode"
    }

    fn proven(&self) -> bool {
        false
    }

    fn check(&self, _model: Option<&str>) -> Check {
        let mut cmd = Command::new("opencode");
        cmd.args(["providers", "list"]);
        let auth = match crate::spawn::run_capture(&mut cmd, b"", 30) {
            Ok(SpawnOutcome::Exited(o)) => o.status.success(),
            _ => false,
        };

        let mut cmd = Command::new("opencode");
        cmd.args(["models"]);
        match crate::spawn::run_capture(&mut cmd, b"", 60) {
            Ok(SpawnOutcome::Exited(o)) if o.status.success() => Check {
                available: auth,
                models: parse_models(&o.stdout),
                error: None,
                proven: false,
            },
            Ok(SpawnOutcome::Exited(o)) => Check {
                available: false,
                models: Vec::new(),
                error: Some(String::from_utf8_lossy(&o.stderr).trim().to_string()),
                proven: false,
            },
            _ => Check {
                available: false,
                models: Vec::new(),
                error: Some("opencode models did not respond".into()),
                proven: false,
            },
        }
    }

    fn probe(&self, model: Option<&str>) -> bool {
        let mut cmd = Command::new("opencode");
        cmd.args(["run", "--format", "json", "--auto"]);
        if let Some(m) = model {
            cmd.args(["--model", m]);
        }
        match crate::spawn::run_capture(&mut cmd, b"ok", 120) {
            Ok(SpawnOutcome::Exited(o)) => o.status.success(),
            _ => false,
        }
    }

    fn run(&self, prompt: &str, cwd: &Path, log: &Path, p: &RunParams) -> io::Result<RunOutcome> {
        let start = Instant::now();
        let mut cmd = Command::new("opencode");
        cmd.current_dir(cwd);
        cmd.args(["run", "--format", "json", "--auto"]);
        cmd.args(["--model", p.model]);
        if !p.effort.is_empty() {
            cmd.args(["--variant", p.effort]);
        }
        let outcome = crate::spawn::run_to_file(&mut cmd, prompt.as_bytes(), log, p.timeout_s)?;
        Ok(RunOutcome {
            status: outcome.status_code(),
            log: log.to_path_buf(),
            cost_usd: None,
            cost_source: CostSource::Unknown,
            duration_s: start.elapsed().as_secs(),
            model: p.model.to_string(),
        })
    }

    fn classify(&self, out: &RunOutcome) -> Classification {
        if out.status == Some(0) {
            return Classification::Ok;
        }
        if self.probe(Some(&out.model)) {
            Classification::TaskFailure
        } else {
            Classification::ProviderUnavailable
        }
    }
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
