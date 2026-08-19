mod common;

use autopilot::label::{label_for_project, repo_slug_for_project};
use common::{make_repo, tmpdir};
use std::process::Command;

fn set_origin(repo: &std::path::Path, url: &str) {
    let status = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["remote", "add", "origin", url])
        .status()
        .expect("git remote");
    assert!(status.success());
}

#[test]
fn https_remote_yields_owner_repo() {
    let repo = make_repo();
    set_origin(&repo, "https://github.com/acme/api.git");
    assert_eq!(repo_slug_for_project(&repo), Some("acme/api".into()));
    assert_eq!(label_for_project(&repo), "com.autopilot.acme-api");
}

#[test]
fn ssh_remote_yields_owner_repo() {
    let repo = make_repo();
    set_origin(&repo, "git@github.com:other/api.git");
    assert_eq!(repo_slug_for_project(&repo), Some("other/api".into()));
    assert_eq!(label_for_project(&repo), "com.autopilot.other-api");
}

#[test]
fn no_remote_fails_the_slug() {
    let repo = make_repo();
    assert_eq!(repo_slug_for_project(&repo), None);
}

#[test]
fn two_remoteless_projects_with_one_name_get_different_labels() {
    let base = tmpdir();
    let one = base.join("one/local");
    let two = base.join("two/local");
    std::fs::create_dir_all(&one).expect("mkdir");
    std::fs::create_dir_all(&two).expect("mkdir");
    for d in [&one, &two] {
        let status = Command::new("git")
            .arg("-C")
            .arg(d)
            .arg("init")
            .arg("-q")
            .status()
            .expect("init");
        assert!(status.success());
    }
    let a = label_for_project(&one);
    let b = label_for_project(&two);
    assert!(a.starts_with("com.autopilot.local-"));
    assert_ne!(
        a, b,
        "same directory name must still produce distinct labels"
    );
}
