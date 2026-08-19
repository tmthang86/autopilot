//! `autopilot preflight`: report what harnesses and models this machine can
//! reach, and which declared tiers do not resolve. One detector, in the
//! adapters — the skill reads this JSON instead of running the CLIs itself.

use serde::Serialize;
use std::path::Path;

use crate::config::Config;

#[derive(Serialize)]
pub struct HarnessReport {
    pub name: String,
    pub available: bool,
    pub models: Vec<String>,
    pub error: Option<String>,
    pub proven: bool,
}

#[derive(Serialize)]
pub struct PreflightReport {
    pub harnesses: Vec<HarnessReport>,
    pub tiers_declared: Vec<String>,
    pub bindings_present: Vec<String>,
    pub unresolved: Vec<String>,
}

pub fn run(project: &Path) -> PreflightReport {
    let cfg = Config::load(project).ok();
    let mut harnesses = Vec::new();
    for h in crate::harness::registry() {
        let check = h.check(None);
        harnesses.push(HarnessReport {
            name: h.name().to_string(),
            available: check.available,
            models: check.models,
            error: check.error,
            proven: h.proven(),
        });
    }
    let (tiers_declared, bindings_present, unresolved) = match &cfg {
        Some(c) => resolve(c),
        None => (Vec::new(), Vec::new(), Vec::new()),
    };
    PreflightReport {
        harnesses,
        tiers_declared,
        bindings_present,
        unresolved,
    }
}

pub fn unresolved(cfg: &Config) -> Vec<String> {
    resolve(cfg).2
}

/// The single check both preflight and guard_tiers use: a tier is reachable
/// only when its binding exists, its harness reports available, and — when the
/// harness enumerates models — its model is in that list.
pub fn tier_ok(cfg: &Config, tier: &str) -> bool {
    let Some(binding) = cfg.bindings.get(tier) else {
        return false;
    };
    let Some(harness) = crate::harness::by_name(&binding.harness) else {
        return false;
    };
    if !harness.available(Some(&binding.model)) {
        return false;
    }
    let models = harness.models();
    models.is_empty() || models.contains(&binding.model)
}

fn resolve(cfg: &Config) -> (Vec<String>, Vec<String>, Vec<String>) {
    let tiers = cfg.tiers.clone();
    let bindings_present = cfg.bindings.keys().cloned().collect();
    let mut unresolved = Vec::new();
    for tier in &tiers {
        if !tier_ok(cfg, tier) {
            unresolved.push(tier.clone());
        }
    }
    (tiers, bindings_present, unresolved)
}

pub fn print(report: &PreflightReport) {
    println!(
        "{}",
        serde_json::to_string_pretty(report).unwrap_or_else(|_| "{}".into())
    );
}
