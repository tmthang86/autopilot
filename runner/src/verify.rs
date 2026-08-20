//! The verification gate. Nothing gets committed unless every configured
//! command passes. An empty list is refused rather than treated as success:
//! with no commands configured, "verified" would mean nothing at all.

use std::path::Path;

use crate::config::Config;
use crate::spawn::SpawnOutcome;

pub struct VerifyFailure {
    pub name: String,
    pub output: String,
}

pub fn run(cfg: &Config, project: &Path) -> Result<(), VerifyFailure> {
    if cfg.verify.is_empty() {
        crate::log::error(
            "no verify commands configured — refusing to treat unverified work as done",
        );
        return Err(VerifyFailure {
            name: "verify".into(),
            output: "no verify commands configured".into(),
        });
    }

    for v in &cfg.verify {
        crate::log::info(&format!("verify: {}", v.name));
        let mut cmd = std::process::Command::new("sh");
        cmd.arg("-c").arg(&v.cmd).current_dir(project);
        match crate::spawn::run_capture(&mut cmd, b"", cfg.pipeline.verify_timeout_s) {
            Ok(SpawnOutcome::Exited(o)) if o.status.success() => {}
            Ok(SpawnOutcome::Exited(o)) => {
                let mut output = String::from_utf8_lossy(&o.stderr).into_owned();
                output.push_str(&String::from_utf8_lossy(&o.stdout));
                crate::log::error(&format!("verify failed at: {}", v.name));
                return Err(VerifyFailure {
                    name: v.name.clone(),
                    output,
                });
            }
            _ => {
                crate::log::error(&format!("verify timed out at: {}", v.name));
                return Err(VerifyFailure {
                    name: v.name.clone(),
                    output: "command timed out".into(),
                });
            }
        }
    }
    Ok(())
}
