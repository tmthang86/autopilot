//! Run state: the runner's only memory between invocations. Every write must be
//! readable back into the same shape, and a stored zero must round-trip as zero
//! rather than being confused with an absent field.
//!
//! The per-issue attempt counter (decision 36) lives here under `attempts`, and
//! is pruned against the open queue so it cannot grow without bound.

use serde_json::{json, Map, Value};
use std::collections::HashSet;
use std::io;
use std::path::{Path, PathBuf};

pub struct State {
    path: PathBuf,
    data: Map<String, Value>,
}

fn default_map() -> Map<String, Value> {
    let mut m = Map::new();
    m.insert("resume_after".into(), json!(0));
    m.insert("backoff_step".into(), json!(0));
    m.insert("consecutive_failures".into(), json!(0));
    m.insert("tasks_today".into(), json!(0));
    m.insert("tasks_today_date".into(), json!(""));
    m.insert("last_issue".into(), json!(0));
    m.insert("attempts".into(), json!({}));
    m
}

impl State {
    pub fn init(path: &Path) -> io::Result<State> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let data = match std::fs::read_to_string(path) {
            Ok(s) => match serde_json::from_str::<Map<String, Value>>(&s) {
                Ok(m) => m,
                Err(_) => {
                    crate::log::warn("state file unreadable, rebuilding from defaults");
                    default_map()
                }
            },
            Err(_) => default_map(),
        };
        Ok(State {
            path: path.to_path_buf(),
            data,
        })
    }

    pub fn get_num(&self, key: &str, default: i64) -> i64 {
        match self.data.get(key) {
            Some(Value::Number(n)) => n.as_i64().unwrap_or(default),
            _ => default,
        }
    }

    pub fn get_str(&self, key: &str, default: &str) -> String {
        match self.data.get(key) {
            Some(Value::String(s)) => s.clone(),
            _ => default.to_string(),
        }
    }

    pub fn set_num(&mut self, key: &str, v: i64) {
        self.data.insert(key.into(), json!(v));
    }

    pub fn set_str(&mut self, key: &str, v: &str) {
        self.data.insert(key.into(), json!(v));
    }

    pub fn bump(&mut self, key: &str) -> i64 {
        let v = self.get_num(key, 0) + 1;
        self.set_num(key, v);
        v
    }

    pub fn save(&self) -> io::Result<()> {
        let tmp = self.path.with_extension("json.tmp");
        let s = serde_json::to_string_pretty(&Value::Object(self.data.clone()))
            .map_err(io::Error::other)?;
        std::fs::write(&tmp, s)?;
        std::fs::rename(&tmp, &self.path)
    }

    pub fn attempt_count(&self, issue: u64) -> u32 {
        self.data
            .get("attempts")
            .and_then(|a| a.get(issue.to_string()))
            .and_then(|n| n.as_u64())
            .unwrap_or(0) as u32
    }

    pub fn record_attempt(&mut self, issue: u64) -> u32 {
        let n = self.attempt_count(issue) + 1;
        let obj = self.ensure_attempts();
        obj.insert(issue.to_string(), json!(n));
        n
    }

    pub fn clear_attempt(&mut self, issue: u64) {
        if let Some(a) = self.data.get_mut("attempts") {
            if let Some(o) = a.as_object_mut() {
                o.remove(&issue.to_string());
            }
        }
    }

    pub fn prune_attempts(&mut self, open: &[u64]) {
        let keep: HashSet<String> = open.iter().map(|n| n.to_string()).collect();
        if let Some(a) = self.data.get_mut("attempts") {
            if let Some(o) = a.as_object_mut() {
                o.retain(|k, _| keep.contains(k));
            }
        }
    }

    fn ensure_attempts(&mut self) -> &mut Map<String, Value> {
        // The file is untrusted input of a sort: a crash or a hand edit can
        // leave a valid JSON `attempts` that is not an object, and the runner
        // must not panic mid-task on it — Rule Zero. Rebuild the field as an
        // empty object rather than assuming the shape.
        if !self.data.get("attempts").is_some_and(|a| a.is_object()) {
            self.data.insert("attempts".into(), json!({}));
        }
        self.data
            .get_mut("attempts")
            .and_then(|a| a.as_object_mut())
            .expect("attempts is an object")
    }
}
