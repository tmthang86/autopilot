mod common;

use autopilot::intent::{self, IntentError};
use common::make_repo;

#[test]
fn a_valid_pointer_is_accepted() {
    let repo = make_repo();
    std::fs::create_dir_all(repo.join("docs/plans")).expect("mkdir");
    std::fs::write(repo.join("docs/plans/0001.md"), "the plan").expect("write");
    let out = intent::resolve("Intent: docs/plans/0001.md", &repo, "Intent:").expect("resolve");
    assert_eq!(out.len(), 1);
    assert!(out[0].ends_with("docs/plans/0001.md"));
}

#[test]
fn the_marker_is_found_wherever_it_appears() {
    let repo = make_repo();
    std::fs::write(repo.join("api.md"), "x").expect("write");
    let out = intent::resolve("Some context.\n\nIntent: api.md\n\nRest.", &repo, "Intent:")
        .expect("resolve");
    assert_eq!(out.len(), 1);
}

#[test]
fn multiple_pointers_on_one_line_are_accepted() {
    let repo = make_repo();
    std::fs::create_dir_all(repo.join("docs/plans")).expect("mkdir");
    std::fs::write(repo.join("docs/plans/0001.md"), "a").expect("write");
    std::fs::write(repo.join("api.md"), "b").expect("write");
    let out =
        intent::resolve("Intent: api.md docs/plans/0001.md", &repo, "Intent:").expect("resolve");
    assert_eq!(out.len(), 2);
}

#[test]
fn a_custom_marker_is_honoured() {
    let repo = make_repo();
    std::fs::write(repo.join("api.md"), "x").expect("write");
    let out = intent::resolve("Source: api.md", &repo, "Source:").expect("resolve");
    assert_eq!(out.len(), 1);
}

#[test]
fn a_task_naming_no_intent_is_refused() {
    let repo = make_repo();
    assert_eq!(
        intent::resolve("Just a body", &repo, "Intent:"),
        Err(IntentError::NoMarker)
    );
}

#[test]
fn a_marker_listing_no_paths_is_refused() {
    let repo = make_repo();
    assert_eq!(
        intent::resolve("Intent:", &repo, "Intent:"),
        Err(IntentError::NoPaths)
    );
}

#[test]
fn an_absolute_path_is_refused() {
    let repo = make_repo();
    assert!(matches!(
        intent::resolve("Intent: /etc/passwd", &repo, "Intent:"),
        Err(IntentError::Absolute(_))
    ));
}

#[test]
fn a_dotdot_component_is_refused() {
    let repo = make_repo();
    assert!(matches!(
        intent::resolve("Intent: ../outside.md", &repo, "Intent:"),
        Err(IntentError::Upward(_))
    ));
}

#[test]
fn a_nonexistent_file_is_refused() {
    let repo = make_repo();
    assert!(matches!(
        intent::resolve("Intent: docs/plans/9999.md", &repo, "Intent:"),
        Err(IntentError::Missing(_))
    ));
}

#[test]
fn a_directory_is_refused() {
    let repo = make_repo();
    std::fs::create_dir_all(repo.join("docs")).expect("mkdir");
    assert!(matches!(
        intent::resolve("Intent: docs", &repo, "Intent:"),
        Err(IntentError::NotFile(_))
    ));
}

#[test]
fn a_symlink_escaping_the_root_is_refused() {
    let repo = make_repo();
    let outside = repo.parent().expect("parent").join("outside.md");
    std::fs::write(&outside, "secret").expect("write");
    std::os::unix::fs::symlink(&outside, repo.join("leak.md")).expect("symlink");
    assert!(matches!(
        intent::resolve("Intent: leak.md", &repo, "Intent:"),
        Err(IntentError::Escapes(_))
    ));
}

#[test]
fn a_symlink_resolving_inside_the_root_is_accepted() {
    let repo = make_repo();
    std::fs::create_dir_all(repo.join("docs/plans")).expect("mkdir");
    std::fs::write(repo.join("docs/plans/0001.md"), "plan").expect("write");
    std::os::unix::fs::symlink("docs/plans/0001.md", repo.join("alias.md")).expect("symlink");
    assert!(intent::resolve("Intent: alias.md", &repo, "Intent:").is_ok());
}
