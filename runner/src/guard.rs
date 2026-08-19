//! Guards. Each answers one question: should this run proceed? Standing down is
//! a normal, quiet outcome, not an error.

use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::state::State;

pub enum GuardOutcome {
    StandDown(String),
}

pub struct Lock {
    dir: PathBuf,
}

impl Lock {
    pub fn acquire(ap: &Path) -> Result<Lock, String> {
        let dir = ap.join("lock");
        if std::fs::create_dir(&dir).is_ok() {
            std::fs::write(dir.join("pid"), std::process::id().to_string()).ok();
            return Ok(Lock { dir });
        }
        let pid = std::fs::read_to_string(dir.join("pid"))
            .unwrap_or_default()
            .trim()
            .parse::<i32>()
            .unwrap_or(0);
        if pid != 0 && alive(pid) {
            return Err(format!("another run is in progress (pid {pid})"));
        }
        std::fs::remove_dir_all(&dir).ok();
        if std::fs::create_dir(&dir).is_ok() {
            std::fs::write(dir.join("pid"), std::process::id().to_string()).ok();
            Ok(Lock { dir })
        } else {
            Err("could not acquire lock".into())
        }
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn alive(pid: i32) -> bool {
    unsafe { libc::kill(pid, 0) == 0 }
}

pub fn project_present(project: &Path) -> Result<(), GuardOutcome> {
    if project.join(".git").is_dir() {
        Ok(())
    } else {
        Err(GuardOutcome::StandDown(format!(
            "project root unavailable: {}",
            project.display()
        )))
    }
}

pub fn stop_file(ap: &Path) -> Result<(), GuardOutcome> {
    if ap.join("STOP").exists() {
        Err(GuardOutcome::StandDown(
            "STOP file present, standing down".into(),
        ))
    } else {
        Ok(())
    }
}

pub fn resume_after(state: &State) -> Result<(), GuardOutcome> {
    let until = state.get_num("resume_after", 0);
    let now = crate::log::unix_now() as i64;
    if until > now {
        Err(GuardOutcome::StandDown(format!(
            "usage window closed for another {}s",
            until - now
        )))
    } else {
        Ok(())
    }
}

pub fn daily_cap(cfg: &Config, state: &mut State) -> Result<(), GuardOutcome> {
    let today = crate::log::date_today();
    if state.get_str("tasks_today_date", "") != today {
        state.set_str("tasks_today_date", &today);
        state.set_num("tasks_today", 0);
    }
    let done = state.get_num("tasks_today", 0);
    let cap = cfg.pacing.daily_task_cap;
    if done >= cap {
        Err(GuardOutcome::StandDown(format!(
            "daily cap reached ({done}/{cap})"
        )))
    } else {
        Ok(())
    }
}

pub fn current_hhmm() -> String {
    match time::OffsetDateTime::now_local() {
        Ok(now) => format!("{:02}{:02}", now.hour(), now.minute()),
        Err(_) => String::from("0000"),
    }
}

pub fn quiet_hours(cfg: &Config, now_hm: &str) -> Result<(), GuardOutcome> {
    for range in &cfg.pacing.quiet_hours {
        if inside(range, now_hm) {
            return Err(GuardOutcome::StandDown(format!(
                "inside quiet hours {range}"
            )));
        }
    }
    Ok(())
}

fn inside(range: &str, now: &str) -> bool {
    let Some((from, to)) = range.split_once('-') else {
        return false;
    };
    let f = hhmm(from);
    let t = hhmm(to);
    let n = hhmm(now);
    if f <= t {
        n >= f && n <= t
    } else {
        n >= f || n <= t
    }
}

fn hhmm(s: &str) -> u32 {
    let s = s.replace(':', "");
    let s = s.trim_start_matches('0');
    if s.is_empty() {
        0
    } else {
        s.parse().unwrap_or(0)
    }
}

pub fn tiers(cfg: &Config, needed: &[&str]) -> Result<(), GuardOutcome> {
    for name in needed {
        if !crate::preflight::tier_ok(cfg, name) {
            return Err(GuardOutcome::StandDown(format!(
                "provider unavailable for tier {name}"
            )));
        }
    }
    Ok(())
}

pub fn all(cfg: &Config, state: &mut State, project: &Path) -> Result<Lock, GuardOutcome> {
    project_present(project)?;
    let ap = project.join(".autopilot");
    stop_file(&ap)?;
    resume_after(state)?;
    quiet_hours(cfg, &current_hhmm())?;
    daily_cap(cfg, state)?;
    Lock::acquire(&ap).map_err(GuardOutcome::StandDown)
}
