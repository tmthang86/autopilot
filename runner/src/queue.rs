//! The queue: the only file that knows GitHub exists. `gh` resolves the
//! repository from the working directory, so every call names the repository
//! explicitly — otherwise a run for one project reads, claims, and closes
//! issues in another.

use serde::Deserialize;
use std::path::Path;

use crate::config::Config;
pub use crate::queue_helpers::deps;

#[derive(Debug, Clone, Deserialize)]
struct GhLabel {
    name: String,
}

#[derive(Debug, Clone, Deserialize)]
struct GhIssue {
    number: u64,
    title: String,
    body: Option<String>,
    labels: Vec<GhLabel>,
}

#[derive(Debug, Clone)]
pub struct Issue {
    pub number: u64,
    pub title: String,
    pub body: String,
    pub labels: Vec<String>,
}

#[derive(Debug)]
pub struct Queue {
    pub repo: String,
    cache: Vec<Issue>,
}

#[derive(Debug)]
pub struct QueueError(pub String);

impl std::fmt::Display for QueueError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for QueueError {}

impl Queue {
    pub fn init(project: &Path) -> Result<Queue, QueueError> {
        match crate::label::repo_slug_for_project(project) {
            Some(repo) => Ok(Queue {
                repo,
                cache: Vec::new(),
            }),
            None => Err(QueueError(format!(
                "project has no origin remote (or none that parses into owner/repo): {}",
                project.display()
            ))),
        }
    }

    pub fn load(&mut self, cfg: &Config) {
        let out = crate::gh::run(
            &[
                "issue",
                "list",
                "--repo",
                &self.repo,
                "--state",
                "open",
                "--limit",
                "50",
                "--json",
                "number,title,body,labels,milestone",
                "--label",
                &cfg.queue.ready_label,
            ],
            crate::gh::TIMEOUT_S,
        );
        match out {
            Ok(o) if o.ok() => {
                self.cache = serde_json::from_str::<Vec<GhIssue>>(&o.stdout)
                    .unwrap_or_default()
                    .into_iter()
                    .map(|i| Issue {
                        number: i.number,
                        title: i.title,
                        body: i.body.unwrap_or_default(),
                        labels: i.labels.into_iter().map(|l| l.name).collect(),
                    })
                    .collect();
            }
            Ok(o) => {
                crate::log::error(&format!(
                    "cannot read the queue for {}: {}",
                    self.repo, o.stderr
                ));
                self.cache.clear();
            }
            Err(e) => {
                crate::log::error(&format!("cannot read the queue for {}: {e}", self.repo));
                self.cache.clear();
            }
        }
        if self.cache.is_empty() {
            self.check_ready_label(cfg);
        }
    }

    fn check_ready_label(&self, cfg: &Config) {
        let out = crate::gh::run(
            &[
                "label", "list", "--repo", &self.repo, "--json", "name", "-q", ".[].name",
            ],
            crate::gh::TIMEOUT_S,
        );
        match out {
            Ok(o) if o.ok() => {
                if !o.stdout.lines().any(|l| l.trim() == cfg.queue.ready_label) {
                    crate::log::error(&format!(
                        "ready label \"{}\" does not exist in {} — issues will never be picked up until it is created",
                        cfg.queue.ready_label, self.repo
                    ));
                }
            }
            Ok(o) => crate::log::error(&format!(
                "cannot verify the ready label in {}: {}",
                self.repo, o.stderr
            )),
            Err(e) => crate::log::error(&format!(
                "cannot verify the ready label in {}: {e}",
                self.repo
            )),
        }
    }

    pub fn pick(&self, cfg: &Config) -> Option<u64> {
        'outer: for issue in &self.cache {
            for excl in &cfg.queue.exclude_labels {
                if issue.labels.iter().any(|l| l == excl) {
                    continue 'outer;
                }
            }
            for dep in deps(&issue.body) {
                if !crate::queue_helpers::is_closed(&self.repo, dep) {
                    continue 'outer;
                }
            }
            return Some(issue.number);
        }
        None
    }

    pub fn field(&self, issue: u64, field: &str) -> Option<String> {
        self.cache
            .iter()
            .find(|i| i.number == issue)
            .and_then(|i| match field {
                "title" => Some(i.title.clone()),
                "body" => Some(i.body.clone()),
                _ => None,
            })
    }

    pub fn has_label(&self, issue: u64, label: &str) -> bool {
        self.labels(issue).iter().any(|l| l == label)
    }

    pub fn labels(&self, issue: u64) -> Vec<String> {
        self.cache
            .iter()
            .find(|i| i.number == issue)
            .map(|i| i.labels.clone())
            .unwrap_or_default()
    }

    /// The issues this load actually saw open, for pruning per-issue state
    /// that no longer has a live issue behind it (decision 36).
    pub fn open_issue_numbers(&self) -> Vec<u64> {
        self.cache.iter().map(|i| i.number).collect()
    }

    pub fn claim(&self, issue: u64) -> Result<(), String> {
        crate::queue_helpers::edit_claim_label(&self.repo, issue, "--add-label", "claim")
    }

    pub fn release(&self, issue: u64) -> Result<(), String> {
        crate::queue_helpers::edit_claim_label(&self.repo, issue, "--remove-label", "release")
    }
}
