//! Orphan detection for `autopilot status`: roles that started in the journal
//! and never reported back. Kept out of `ctl.rs` so both modules stay small
//! enough to hold in one head (the 200-line boundary is tested).

use std::path::Path;

/// Every journal file the runner has written, current `journal.jsonl` first,
/// then rolled `journal-<date>.jsonl` files in name order. A roll renames the
/// old file away, so a detection that read only `journal.jsonl` went blind the
/// moment the journal first grew past its size cap.
fn journal_files(ap: &Path) -> Vec<std::path::PathBuf> {
    let mut files = vec![ap.join("journal.jsonl")];
    if let Ok(rd) = std::fs::read_dir(ap) {
        let mut rolled: Vec<_> = rd
            .flatten()
            .filter(|e| {
                let n = e.file_name();
                let n = n.to_string_lossy();
                n.starts_with("journal-") && n.ends_with(".jsonl")
            })
            .map(|e| e.path())
            .collect();
        rolled.sort();
        files.extend(rolled);
    }
    files
}

/// The orphan report as lines, separated from printing so it can be tested.
pub fn orphan_report(project: &Path) -> Vec<String> {
    let mut starts: std::collections::HashMap<(String, String, u32, String), ()> =
        std::collections::HashMap::new();
    let mut ends: std::collections::HashSet<(String, String, u32, String)> =
        std::collections::HashSet::new();
    for path in journal_files(&project.join(".autopilot")) {
        let Ok(body) = std::fs::read_to_string(&path) else {
            continue;
        };
        for line in body.lines() {
            let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
                continue;
            };
            let Some(event) = v.get("event").and_then(|e| e.as_str()) else {
                continue;
            };
            let wake = v.get("wake").and_then(|w| w.as_str()).unwrap_or("");
            let role = v.get("role").and_then(|r| r.as_str()).unwrap_or("");
            let round = v.get("round").and_then(|r| r.as_u64()).unwrap_or(0) as u32;
            let lens = v.get("lens").and_then(|l| l.as_str()).unwrap_or("");
            let key = (wake.to_string(), role.to_string(), round, lens.to_string());
            match event {
                "role_start" => {
                    starts.insert(key.clone(), ());
                }
                "role_end" => {
                    ends.insert(key);
                }
                _ => {}
            }
        }
    }
    let mut out = Vec::new();
    for key in starts.keys() {
        if !ends.contains(key) {
            out.push(format!(
                "orphaned: role {} never finished (wake {})",
                key.1, key.0
            ));
        }
    }
    out
}

/// Print the orphan report (the `status` command surface).
pub fn report(project: &Path) {
    for line in orphan_report(project) {
        println!("{line}");
    }
}
