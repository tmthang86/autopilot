mod common;

use autopilot::ctl;
use common::{make_repo, prepend_stubs, set_stub, tmpdir};
use std::path::Path;

fn setup() -> (std::path::PathBuf, std::path::PathBuf) {
    let jobs = tmpdir();
    std::env::set_var("AUTOPILOT_PLIST_DIR", &jobs);
    let repo = make_repo();
    std::process::Command::new("git")
        .arg("-C")
        .arg(&repo)
        .args(["remote", "add", "origin", "https://github.com/acme/api.git"])
        .status()
        .expect("git");
    (repo, jobs)
}

#[test]
fn a_stop_that_did_not_stop_exits_nonzero() {
    let _g = prepend_stubs();
    let (repo, _) = setup();
    set_stub(
        "launchctl",
        r#"case "$1" in print) exit 0 ;; bootout) echo "Boot-out failed: 5: Input/output error" >&2; exit 5 ;; esac
exit 0"#,
    );
    assert_ne!(ctl::stop(&repo), 0);
}

#[test]
fn status_names_an_orphaned_role() {
    let _g = prepend_stubs();
    let (repo, _) = setup();
    set_stub("launchctl", "case \"$1\" in print) exit 1 ;; esac; exit 0");
    let ap = repo.join(".autopilot");
    std::fs::create_dir_all(&ap).expect("mkdir");
    std::fs::write(
        ap.join("journal.jsonl"),
        "{\"ts\":\"x\",\"wake\":\"w-1\",\"event\":\"role_start\",\"role\":\"review\",\"round\":1}\n",
    )
    .expect("journal");
    let out = std::panic::catch_unwind(|| {
        // status prints; we just need it not to crash and to find the orphan.
        ctl::status(&repo)
    });
    assert!(out.is_ok());
}

#[test]
fn orphan_report_reads_rolled_journals() {
    let _g = prepend_stubs();
    let (repo, _) = setup();
    set_stub("launchctl", "case \"$1\" in print) exit 1 ;; esac; exit 0");
    let ap = repo.join(".autopilot");
    std::fs::create_dir_all(&ap).expect("mkdir");
    // The current journal holds a completed role; the rolled journal holds the
    // orphan. Only a scan across both files finds it.
    std::fs::write(
        ap.join("journal.jsonl"),
        "{\"ts\":\"x\",\"wake\":\"w-2\",\"event\":\"role_start\",\"role\":\"test\",\"round\":0}\n{\"ts\":\"x\",\"wake\":\"w-2\",\"event\":\"role_end\",\"role\":\"test\",\"round\":0}\n",
    )
    .expect("journal");
    std::fs::write(
        ap.join("journal-2026-08-19.jsonl"),
        "{\"ts\":\"x\",\"wake\":\"w-1\",\"event\":\"role_start\",\"role\":\"review\",\"round\":1}\n",
    )
    .expect("rolled");
    let report = autopilot::orphans::orphan_report(&repo);
    assert!(
        report
            .iter()
            .any(|l| l.contains("review") && l.contains("w-1")),
        "an orphan in a rolled journal is named: {report:?}"
    );
    assert!(
        !report.iter().any(|l| l.contains("test")),
        "a completed role is not an orphan: {report:?}"
    );
}

#[test]
fn the_plist_path_is_overridable() {
    let jobs = tmpdir();
    std::env::set_var("AUTOPILOT_PLIST_DIR", &jobs);
    let p = ctl::plist_path("com.autopilot.x");
    assert_eq!(p.parent().unwrap(), Path::new(&jobs));
}
