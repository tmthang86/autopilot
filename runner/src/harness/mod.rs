//! The harness abstraction. A harness is an agentic CLI: it has tools, reads
//! and edits files, runs commands, and commits. A model server is not a
//! harness. This file is the only place outside `harness/*.rs` that may know a
//! harness exists — and the only file that maps a name to an adapter.

use std::io;
use std::path::{Path, PathBuf};

use crate::journal::CostSource;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Classification {
    Ok,
    TaskFailure,
    ProviderUnavailable,
}

#[derive(Debug, Clone)]
pub struct Check {
    pub available: bool,
    pub models: Vec<String>,
    pub error: Option<String>,
    pub proven: bool,
}

pub struct RunParams<'a> {
    pub model: &'a str,
    pub effort: &'a str,
    pub budget_usd: f64,
    pub timeout_s: u64,
    pub permission_mode: &'a str,
}

pub struct RunOutcome {
    pub status: Option<i32>,
    pub log: PathBuf,
    pub cost_usd: Option<f64>,
    pub cost_source: CostSource,
    pub duration_s: u64,
    pub model: String,
}

pub trait Harness {
    fn name(&self) -> &'static str;
    fn proven(&self) -> bool;
    fn check(&self, model: Option<&str>) -> Check;
    fn run(&self, prompt: &str, cwd: &Path, log: &Path, p: &RunParams) -> io::Result<RunOutcome>;
    fn probe(&self, model: Option<&str>) -> bool;
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
    fn available(&self, model: Option<&str>) -> bool {
        self.check(model).available
    }
    fn models(&self) -> Vec<String> {
        self.check(None).models
    }
}

mod claude;
mod opencode;
mod pi;

pub fn by_name(name: &str) -> Option<Box<dyn Harness>> {
    match name {
        "claude" => Some(Box::new(claude::Claude)),
        "pi" => Some(Box::new(pi::Pi)),
        "opencode" => Some(Box::new(opencode::Opencode)),
        _ => None,
    }
}

pub fn registry() -> Vec<Box<dyn Harness>> {
    vec![
        Box::new(claude::Claude),
        Box::new(pi::Pi),
        Box::new(opencode::Opencode),
    ]
}
