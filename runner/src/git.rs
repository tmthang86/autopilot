//! `git` as a child process, through the single spawn door. The runner never
//! links a git library: every finding in `observed-behaviour.md` is about how
//! the CLI actually behaves, and those findings stay valid only if the CLI is
//! what runs.

use std::path::Path;
use std::process::Command;

pub struct GitResult {
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
}

pub fn run(project: &Path, args: &[&str], timeout_s: u64) -> std::io::Result<GitResult> {
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(project).args(args);
    match crate::spawn::run_capture(&mut cmd, b"", timeout_s)? {
        crate::spawn::SpawnOutcome::Exited(o) => Ok(GitResult {
            status: o.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&o.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&o.stderr).into_owned(),
        }),
        crate::spawn::SpawnOutcome::TimedOut => Ok(GitResult {
            status: -1,
            stdout: String::new(),
            stderr: "git timed out".into(),
        }),
    }
}
