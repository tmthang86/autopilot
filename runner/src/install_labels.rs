//! Label creation for `autopilot install`, split out so the installer stays
//! small. The measured rule: only a stderr containing `already exists` is the
//! benign case for `gh label create`.

use std::path::Path;

pub fn home() -> String {
    std::env::var("HOME").unwrap_or_default()
}

pub fn ready_label(cfg_path: &Path) -> String {
    std::fs::read_to_string(cfg_path)
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .and_then(|v| {
            v.pointer("/queue/ready_label")
                .and_then(|l| l.as_str())
                .map(str::to_string)
        })
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "autopilot".into())
}

pub fn declared_tiers(cfg_path: &Path) -> Vec<String> {
    std::fs::read_to_string(cfg_path)
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .map(|v| {
            v.get("tiers")
                .and_then(|t| t.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|t| t.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default()
        })
        .unwrap_or_default()
}

/// Returns true when the label creation genuinely failed (not "already exists").
pub fn create_label(slug: &str, name: &str, color: &str, desc: &str) -> bool {
    let out = crate::gh::run(
        &[
            "label",
            "create",
            name,
            "--repo",
            slug,
            "--color",
            color,
            "--description",
            desc,
        ],
        crate::gh::TIMEOUT_S,
    );
    match out {
        Ok(o) if o.ok() => false,
        Ok(o) => {
            if o.stderr.contains("already exists") {
                false
            } else {
                eprintln!("  {name}: label creation failed: {}", o.stderr);
                true
            }
        }
        Err(e) => {
            eprintln!("  {name}: label creation failed: {e}");
            true
        }
    }
}
