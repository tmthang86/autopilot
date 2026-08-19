mod common;

use autopilot::install;
use common::{make_repo, prepend_stubs, set_stub, tmpdir};

#[test]
fn a_project_with_no_origin_is_refused_before_writing() {
    let _g = prepend_stubs();
    let repo = make_repo();
    set_stub("gh", "exit 0");
    let rc = install::run(&repo, 1800);
    assert_eq!(rc, 1);
    assert!(
        !repo.join(".autopilot/config.json").exists(),
        "nothing was written"
    );
}

#[test]
fn a_full_install_writes_config_gitignore_and_labels() {
    let _g = prepend_stubs();
    let jobs = tmpdir();
    std::env::set_var("AUTOPILOT_PLIST_DIR", &jobs);
    let repo = make_repo();
    std::process::Command::new("git")
        .arg("-C")
        .arg(&repo)
        .args([
            "remote",
            "add",
            "origin",
            "https://github.com/acme/fresh.git",
        ])
        .status()
        .expect("git");
    set_stub("gh", "exit 0");
    assert_eq!(install::run(&repo, 1800), 0);

    assert!(repo.join(".autopilot/config.json").exists());
    let cfg = std::fs::read_to_string(repo.join(".autopilot/config.json")).expect("cfg");
    assert!(
        cfg.contains("\"tiers\""),
        "the config carries the tier ladder"
    );
    let gi = std::fs::read_to_string(repo.join(".gitignore")).expect("gitignore");
    assert!(
        gi.contains(".autopilot/tiers.local.json"),
        "bindings are gitignored"
    );
    let plist =
        std::fs::read_to_string(jobs.join("com.autopilot.acme-fresh.plist")).expect("plist");
    assert!(plist.contains("run-once"), "the plist launches the binary");
    assert!(plist.contains("--project"));
}
