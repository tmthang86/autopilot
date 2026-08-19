//! The launchd job label, derived from the `origin` remote so two projects with
//! the same directory name never share a job.

use std::path::Path;

pub fn repo_slug_for_project(dir: &Path) -> Option<String> {
    let out = crate::git::run(dir, &["remote", "get-url", "origin"], 60).ok()?;
    if out.status != 0 {
        return None;
    }
    parse_slug(out.stdout.trim())
}

fn parse_slug(url: &str) -> Option<String> {
    let mut s = url.trim();
    if let Some(rest) = s.strip_prefix("git@") {
        let idx = rest.find(':')?;
        s = &rest[idx + 1..];
    } else {
        for scheme in ["https://", "http://"] {
            if let Some(rest) = s.strip_prefix(scheme) {
                s = rest;
                break;
            }
        }
        let idx = s.find('/')?;
        s = &s[idx + 1..];
    }
    let s = s.trim_end_matches(".git");
    if s.contains('/') {
        Some(s.to_string())
    } else {
        None
    }
}

pub fn label_for_project(dir: &Path) -> String {
    if let Some(slug) = repo_slug_for_project(dir) {
        return format!("com.autopilot.{}", slug.replace('/', "-"));
    }
    let abs = std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
    let name = abs
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "project".into());
    format!("com.autopilot.{name}-{}", digest(&abs.to_string_lossy()))
}

fn digest(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    h
}
