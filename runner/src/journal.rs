//! The run journal: one append-only JSONL file per project.
//!
//! A `role_start` with no matching `role_end` is the signature of a role that
//! died silently — the failure the whole journal exists to detect. The file is
//! flushed on every event so a killed process still leaves the start line.

use serde::Serialize;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

#[derive(Serialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum CostSource {
    Reported,
    Unknown,
}

#[derive(Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Event {
    WakeStart {
        issue: Option<u64>,
        tier: Option<String>,
    },
    RoleStart {
        role: String,
        round: u32,
        tier: String,
        harness: String,
        model: String,
        lens: Option<String>,
    },
    RoleEnd {
        role: String,
        round: u32,
        tier: String,
        classify: String,
        cost_usd: Option<f64>,
        cost_source: CostSource,
        duration_s: u64,
        lens: Option<String>,
    },
    Verdict {
        role: String,
        round: u32,
        verdict: String,
        reason: String,
    },
    Verify {
        result: String,
        failed: Option<String>,
    },
    WakeEnd {
        outcome: String,
    },
}

pub struct Journal {
    file: fs::File,
    wake: String,
    path: PathBuf,
}

impl Journal {
    pub fn open(project: &Path, max_bytes: u64) -> io::Result<Journal> {
        let dir = project.join(".autopilot");
        fs::create_dir_all(&dir)?;
        let path = roll_if_needed(dir.join("journal.jsonl"), max_bytes)?;
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        Ok(Journal {
            file,
            wake: crate::log::wake_id(),
            path,
        })
    }

    pub fn wake(&self) -> &str {
        &self.wake
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn append(&mut self, ev: Event) -> io::Result<()> {
        let mut v = serde_json::to_value(&ev).map_err(io::Error::other)?;
        if let Some(o) = v.as_object_mut() {
            o.insert(
                "ts".into(),
                serde_json::Value::String(crate::log::timestamp()),
            );
            o.insert(
                "wake".into(),
                serde_json::Value::String(self.wake.clone()),
            );
        }
        writeln!(self.file, "{v}")?;
        self.file.flush()
    }
}

fn roll_if_needed(path: PathBuf, max_bytes: u64) -> io::Result<PathBuf> {
    let size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    if size <= max_bytes {
        return Ok(path);
    }
    let dir = path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let base = format!("journal-{}", crate::log::date_today());
    let mut candidate = dir.join(format!("{base}.jsonl"));
    let mut i = 1u32;
    while candidate.exists() {
        candidate = dir.join(format!("{base}-{i}.jsonl"));
        i += 1;
    }
    fs::rename(&path, &candidate)?;
    Ok(path)
}
