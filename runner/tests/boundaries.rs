use std::fs;
use std::path::Path;

fn read_all(dir: &Path, out: &mut Vec<(String, String)>) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            read_all(&p, out);
        } else if p.extension().map_or(false, |x| x == "rs") {
            if let Ok(s) = fs::read_to_string(&p) {
                out.push((p.display().to_string(), s));
            }
        }
    }
}

#[test]
fn no_harness_name_outside_the_harness_module() {
    let mut files = Vec::new();
    read_all(Path::new("src"), &mut files);
    let mut offenders = Vec::new();
    for (path, body) in &files {
        if path.contains("src/harness/") {
            continue;
        }
        for name in ["claude", "opencode", "codex"] {
            if body.contains(name) {
                offenders.push(format!("{path}: {name}"));
            }
        }
        // "pi" is too short to grep for; it is matched as a whole word instead.
        for line in body.lines() {
            if line.split(|c: char| !c.is_alphanumeric()).any(|w| w == "pi") {
                offenders.push(format!("{path}: pi"));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "harness names leaked out of src/harness/: {offenders:?}"
    );
}

#[test]
fn the_runner_never_references_a_skill_or_the_dashboard() {
    let mut files = Vec::new();
    read_all(Path::new("src"), &mut files);
    for (path, body) in &files {
        assert!(!body.contains("skills/"), "{path} references skills/");
        assert!(!body.contains("dashboard"), "{path} references the dashboard");
    }
}

#[test]
fn no_module_exceeds_two_hundred_lines() {
    let mut files = Vec::new();
    read_all(Path::new("src"), &mut files);
    let over: Vec<_> = files
        .iter()
        .filter(|(_, b)| b.lines().count() > 200)
        .map(|(p, b)| format!("{p}: {}", b.lines().count()))
        .collect();
    assert!(over.is_empty(), "modules over 200 lines: {over:?}");
}
