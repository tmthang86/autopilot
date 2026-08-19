//! `autopilot status | start | stop`: manage the launchd job and report what it
//! has actually done. `start` and `stop` verify with `launchctl print` rather
//! than assuming; `status` names orphaned roles from the journal.

use std::path::Path;
use std::process::Command;

pub fn plist_path(label: &str) -> std::path::PathBuf {
    let dir = std::env::var("AUTOPILOT_PLIST_DIR").unwrap_or_else(|_| {
        format!(
            "{}/.local/share/autopilot/jobs",
            std::env::var("HOME").unwrap_or_default()
        )
    });
    std::path::PathBuf::from(dir).join(format!("{label}.plist"))
}

pub fn label(project: &Path) -> String {
    crate::label::label_for_project(project)
}

pub fn target() -> String {
    format!("gui/{}", unsafe { libc::getuid() })
}

fn launchctl(args: &[&str]) -> std::process::Output {
    let mut cmd = Command::new("launchctl");
    cmd.args(args);
    match crate::spawn::run_capture(&mut cmd, b"", 60) {
        Ok(crate::spawn::SpawnOutcome::Exited(o)) => o,
        _ => std::process::Output {
            status: Default::default(),
            stdout: Vec::new(),
            stderr: Vec::new(),
        },
    }
}

fn loaded(label: &str) -> bool {
    launchctl(&["print", &format!("{}/{}", target(), label)])
        .status
        .success()
}

pub fn start(project: &Path) -> i32 {
    let label = label(project);
    let plist = plist_path(&label);
    if !plist.exists() {
        eprintln!(
            "no job installed for {} — run autopilot install first",
            project.display()
        );
        return 1;
    }
    let t = format!("{}/{}", target(), label);
    launchctl(&["enable", &t]);
    launchctl(&["bootstrap", &t, plist.to_str().unwrap_or_default()]);
    if loaded(&label) {
        println!("autopilot started for {}", project.display());
        0
    } else {
        eprintln!("FAILED to start autopilot for {}", project.display());
        eprintln!("  job file: {}", plist.display());
        1
    }
}

pub fn stop(project: &Path) -> i32 {
    let label = label(project);
    let was_loaded = loaded(&label);
    let t = format!("{}/{}", target(), label);
    launchctl(&["bootout", &t]);
    launchctl(&["disable", &t]);
    if loaded(&label) {
        eprintln!(
            "FAILED to stop autopilot for {} — the job is still loaded",
            project.display()
        );
        eprintln!("  label: {label}");
        return 1;
    }
    println!(
        "autopilot stopped for {}, and will stay stopped across reboots",
        project.display()
    );
    if !was_loaded {
        println!("note: no job was loaded under {label}. If this project was started before the");
        println!("      job label changed, an older job may still be running — see the");
        println!("      upgrade section of docs/guides/install.md.");
    }
    0
}

pub fn status(project: &Path) -> i32 {
    let label = label(project);
    let plist = plist_path(&label);
    println!("project:  {}", project.display());
    println!(
        "job file: {}",
        if plist.exists() {
            plist.display().to_string()
        } else {
            "not installed".into()
        }
    );
    println!("runner:   {}", env!("CARGO_PKG_VERSION"));
    println!("loaded:   {}", if loaded(&label) { "yes" } else { "no" });

    let ap = project.join(".autopilot");
    if ap.join("STOP").exists() {
        println!("STOP:     present — the runner will stand down at every tick");
    }
    let state = crate::state::State::init(&ap.join("state.json")).ok();
    let resume = state
        .as_ref()
        .map(|s| s.get_num("resume_after", 0))
        .unwrap_or(0);
    if resume > crate::log::unix_now() as i64 {
        println!(
            "paused:   usage window closed for another {}s",
            resume - crate::log::unix_now() as i64
        );
    }
    println!(
        "today:    {} tasks",
        state
            .as_ref()
            .map(|s| s.get_num("tasks_today", 0))
            .unwrap_or(0)
    );
    report_orphans(project);
    0
}

fn report_orphans(project: &Path) {
    let path = project.join(".autopilot/journal.jsonl");
    let body = match std::fs::read_to_string(&path) {
        Ok(b) => b,
        Err(_) => return,
    };
    let mut starts: std::collections::HashMap<(String, String, u32, String), ()> =
        std::collections::HashMap::new();
    let mut ends: std::collections::HashSet<(String, String, u32, String)> =
        std::collections::HashSet::new();
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
    for key in starts.keys() {
        if !ends.contains(key) {
            println!("orphaned: role {} never finished (wake {})", key.1, key.0);
        }
    }
}
