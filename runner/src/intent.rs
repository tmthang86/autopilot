//! Intent binding. Every task names the documents that authorise it; the runner
//! validates that pointer before spending a token. The issue body is untrusted
//! text, and the runner runs with permission checks disabled, so containment
//! here is a security boundary, not a style check.

use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntentError {
    NoMarker,
    NoPaths,
    Absolute(String),
    Upward(String),
    Missing(String),
    NotFile(String),
    Escapes(String),
    BadRoot,
}

impl fmt::Display for IntentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IntentError::NoMarker => write!(
                f,
                "task names no intent — it must carry an 'Intent:' line naming the documents that authorise it"
            ),
            IntentError::NoPaths => {
                write!(f, "task names no intent — the 'Intent:' line lists no paths")
            }
            IntentError::Absolute(p) => write!(f, "intent path is absolute, refusing: {p}"),
            IntentError::Upward(p) => write!(f, "intent path traverses upward, refusing: {p}"),
            IntentError::Missing(p) => write!(f, "intent path does not exist: {p}"),
            IntentError::NotFile(p) => write!(f, "intent path is not a file: {p}"),
            IntentError::Escapes(p) => {
                write!(f, "intent path resolves outside the project root: {p}")
            }
            IntentError::BadRoot => write!(f, "cannot resolve project root"),
        }
    }
}

impl std::error::Error for IntentError {}

pub fn resolve(body: &str, project: &Path, marker: &str) -> Result<Vec<PathBuf>, IntentError> {
    let mut found = false;
    let mut paths: Vec<String> = Vec::new();
    for line in body.lines() {
        let t = line.trim_start();
        if let Some(rest) = t.strip_prefix(marker) {
            found = true;
            let rest = rest.trim();
            if rest.is_empty() {
                return Err(IntentError::NoPaths);
            }
            paths = rest.split_whitespace().map(str::to_string).collect();
            break;
        }
    }
    if !found {
        return Err(IntentError::NoMarker);
    }

    let root = std::fs::canonicalize(project).map_err(|_| IntentError::BadRoot)?;
    let mut resolved = Vec::new();
    for p in &paths {
        if p.starts_with('/') {
            return Err(IntentError::Absolute(p.clone()));
        }
        if format!("/{p}/").contains("/../") {
            return Err(IntentError::Upward(p.clone()));
        }
        let full = root.join(p);
        if !full.exists() {
            return Err(IntentError::Missing(p.clone()));
        }
        let canon = std::fs::canonicalize(&full).map_err(|_| IntentError::Missing(p.clone()))?;
        if !canon.is_file() {
            return Err(IntentError::NotFile(p.clone()));
        }
        if !canon.starts_with(&root) {
            return Err(IntentError::Escapes(p.clone()));
        }
        resolved.push(canon);
    }
    if resolved.is_empty() {
        return Err(IntentError::NoPaths);
    }
    Ok(resolved)
}
