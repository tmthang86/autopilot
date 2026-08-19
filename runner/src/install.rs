//! `autopilot install`: prepare a project — config, gitignore, the launchd
//! job, and the labels the queue queries. It starts nothing, and it refuses a
//! project with no `origin` before writing anything.

use std::path::Path;

use crate::install_labels::{create_label, declared_tiers, home, ready_label};

const CONFIG_TEMPLATE: &str = include_str!("../templates/config-tiered.json");
const PLIST_TEMPLATE: &str = include_str!("../templates/launchd-binary.plist.tmpl");

pub fn run(project: &Path, interval: u64) -> i32 {
    if !project.join(".git").is_dir() {
        eprintln!("not a git repository: {}", project.display());
        return 1;
    }
    let Some(slug) = crate::label::repo_slug_for_project(project) else {
        eprintln!(
            "no origin remote (or none that parses into owner/repo): {}",
            project.display()
        );
        eprintln!("Add an origin remote and run this again. Nothing was written.");
        return 1;
    };

    let name = project
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "project".into());
    let label = crate::label::label_for_project(project);
    let ap = project.join(".autopilot");
    std::fs::create_dir_all(ap.join("logs")).ok();

    let cfg_path = ap.join("config.json");
    if !cfg_path.exists() {
        let cfg = CONFIG_TEMPLATE.replace("CHANGE_ME", &name);
        if std::fs::write(&cfg_path, cfg).is_err() {
            eprintln!("could not write {}", cfg_path.display());
            return 1;
        }
        println!(
            "created {} — edit its verify commands before starting",
            cfg_path.display()
        );
    } else {
        println!("keeping existing {}", cfg_path.display());
    }

    let gitignore = project.join(".gitignore");
    let mut entries = Vec::new();
    if let Ok(existing) = std::fs::read_to_string(&gitignore) {
        entries = existing.lines().map(str::to_string).collect();
    }
    for entry in [
        ".autopilot/state.json",
        ".autopilot/logs/",
        ".autopilot/STOP",
        ".autopilot/lock/",
        ".autopilot/tiers.local.json",
        ".autopilot/runs/",
        ".autopilot/journal.jsonl",
    ] {
        if !entries.iter().any(|e| e == entry) {
            entries.push(entry.to_string());
        }
    }
    let mut body = entries.join("\n");
    if !body.is_empty() {
        body.push('\n');
    }
    if std::fs::write(&gitignore, body).is_err() {
        eprintln!("could not write {}", gitignore.display());
        return 1;
    }

    let ready_label = ready_label(&cfg_path);
    let mut label_failures = 0;
    let fixed: &[(&str, &str, &str)] = &[
        (
            "needs-human",
            "fbca04",
            "Correctness needs a person to observe it; autopilot must not close",
        ),
        ("blocked", "d93f0b", "Waiting on a human decision"),
        ("status:in-progress", "1d76db", "Claimed by a run"),
    ];
    for (l, color, desc) in fixed {
        if create_label(&slug, l, color, desc) {
            label_failures += 1;
        }
    }
    if !fixed.iter().any(|(l, _, _)| l == &ready_label)
        && create_label(
            &slug,
            &ready_label,
            "0e8a16",
            "Eligible for unattended execution",
        )
    {
        label_failures += 1;
    }
    for tier in declared_tiers(&cfg_path) {
        create_label(
            &slug,
            &format!("tier:{tier}"),
            "c5def5",
            &format!("Run on the {tier} tier"),
        );
    }

    let plist_dir = std::env::var("AUTOPILOT_PLIST_DIR")
        .unwrap_or_else(|_| format!("{}/.local/share/autopilot/jobs", home()));
    std::fs::create_dir_all(&plist_dir).ok();
    let runner = std::env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "autopilot".into());
    let plist = PLIST_TEMPLATE
        .replace("{{LABEL}}", &label)
        .replace("{{RUNNER}}", &runner)
        .replace("{{PROJECT}}", &project.display().to_string())
        .replace("{{INTERVAL}}", &interval.to_string())
        .replace("{{HOME}}", &home());
    let plist_path = std::path::PathBuf::from(&plist_dir).join(format!("{label}.plist"));
    if std::fs::write(&plist_path, plist).is_err() {
        eprintln!("could not write {}", plist_path.display());
        return 1;
    }

    if label_failures > 0 {
        eprintln!("warning: {label_failures} label(s) could not be created — see errors above. Fix and re-run before starting.");
        return 1;
    }

    println!();
    println!(
        "Wrote {} (deliberately not in ~/Library/LaunchAgents, which launchd auto-loads)",
        plist_path.display()
    );
    println!(
        "The job is NOT running. Review {} first, then:",
        cfg_path.display()
    );
    println!();
    println!("  autopilot start {}", project.display());
    println!();
    println!("Stop it again with:");
    println!();
    println!("  autopilot stop {}", project.display());
    0
}
