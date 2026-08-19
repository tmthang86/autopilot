//! Role verdicts, written to files by the harness rather than parsed from
//! stdout. A missing or malformed verdict is a task failure, never a pass. The
//! verdict directory is created by the runner immediately before a role runs,
//! and a verdict file that already exists for that role is refused — the
//! reviewed party must not be able to author its own review (decision 35).

use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Verdict {
    pub verdict: String,
    pub reason: String,
    #[serde(default)]
    pub evidence: Vec<String>,
}

impl Verdict {
    pub fn is_pass(&self) -> bool {
        self.verdict == "pass"
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum VerdictError {
    Missing,
    Malformed(String),
}

impl std::fmt::Display for VerdictError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VerdictError::Missing => write!(f, "verdict file is missing"),
            VerdictError::Malformed(e) => write!(f, "verdict file is malformed: {e}"),
        }
    }
}

impl std::error::Error for VerdictError {}

pub fn read(path: &Path) -> Result<Verdict, VerdictError> {
    let body = std::fs::read_to_string(path).map_err(|_| VerdictError::Missing)?;
    serde_json::from_str(&body).map_err(|e| VerdictError::Malformed(e.to_string()))
}

pub fn dir_for(project: &Path, issue: u64, wake: &str, attempt: u32, kind: &str) -> PathBuf {
    project
        .join(".autopilot/runs")
        .join(issue.to_string())
        .join(wake)
        .join(format!("attempt-{attempt}"))
        .join(kind)
}

pub fn verdict_path(dir: &Path, role: &str) -> PathBuf {
    dir.join(format!("{role}-verdict.json"))
}

/// Create the verdict directory immediately before a role runs and refuse if
/// that role's verdict file already exists there.
pub fn prepare(
    project: &Path,
    issue: u64,
    wake: &str,
    attempt: u32,
    kind: &str,
    role: &str,
) -> Result<PathBuf, String> {
    let dir = dir_for(project, issue, wake, attempt, kind);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = verdict_path(&dir, role);
    if path.exists() {
        return Err(format!(
            "refusing to run: {} already exists before the role was invoked",
            path.display()
        ));
    }
    Ok(dir)
}
