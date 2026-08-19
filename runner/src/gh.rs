//! `gh` as a child process. Every call names the repository explicitly, and a
//! caller that must react gets gh's stderr back rather than having it discarded.

use std::process::Command;

pub struct GhResult {
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
}

impl GhResult {
    pub fn ok(&self) -> bool {
        self.status == 0
    }
}

pub fn run(args: &[&str], timeout_s: u64) -> std::io::Result<GhResult> {
    let mut cmd = Command::new("gh");
    cmd.args(args);
    match crate::spawn::run_capture(&mut cmd, b"", timeout_s)? {
        crate::spawn::SpawnOutcome::Exited(o) => Ok(GhResult {
            status: o.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&o.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&o.stderr).into_owned(),
        }),
        crate::spawn::SpawnOutcome::TimedOut => Ok(GhResult {
            status: -1,
            stdout: String::new(),
            stderr: "gh timed out".into(),
        }),
    }
}

pub const TIMEOUT_S: u64 = 60;

pub fn comment(repo: &str, issue: u64, body: &str) -> bool {
    run(
        &[
            "issue",
            "comment",
            &issue.to_string(),
            "--repo",
            repo,
            "--body",
            body,
        ],
        TIMEOUT_S,
    )
    .map(|o| o.ok())
    .unwrap_or(false)
}

pub fn add_label(repo: &str, issue: u64, label: &str) -> bool {
    run(
        &[
            "issue",
            "edit",
            &issue.to_string(),
            "--repo",
            repo,
            "--add-label",
            label,
        ],
        TIMEOUT_S,
    )
    .map(|o| o.ok())
    .unwrap_or(false)
}

pub fn close(repo: &str, issue: u64) -> bool {
    run(
        &["issue", "close", &issue.to_string(), "--repo", repo],
        TIMEOUT_S,
    )
    .map(|o| o.ok())
    .unwrap_or(false)
}
