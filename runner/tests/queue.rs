mod common;

use autopilot::config::Config;
use autopilot::queue::{deps, Queue};
use common::{make_repo, prepend_stubs, set_stub};
use std::path::Path;

fn config_json() -> String {
    r#"{
        "project": {"main_branch": "main", "work_branch": "autopilot/main"},
        "queue": {"ready_label": "autopilot", "exclude_labels": ["blocked","needs-human","status:in-progress"], "intent_marker": "Intent:"},
        "tiers": ["light"],
        "roles": {"implement": {"tier_offset": 0}, "test": {"tier_offset": 0}, "review": {"tier_offset": 1}},
        "pipeline": {"max_rounds": 2, "turn_timeout_s": 60, "wake_timeout_s": 600, "wake_budget_usd": 25.0, "review_lenses": []},
        "agent": {"permission_mode": "bypassPermissions", "default_tier": "light"},
        "verify": [{"name": "t", "cmd": "true"}],
        "pacing": {"daily_task_cap": 12, "max_attempts_per_issue": 2, "circuit_breaker_failures": 3},
        "autonomy": {"prepare_only_label": "needs-human"}
    }"#
    .to_string()
}

fn write_config(repo: &Path) -> Config {
    let ap = repo.join(".autopilot");
    std::fs::create_dir_all(&ap).expect("mkdir");
    std::fs::write(ap.join("config.json"), config_json()).expect("cfg");
    std::fs::write(
        ap.join("tiers.local.json"),
        r#"{"light": {"harness": "dummy", "model": "m", "effort": "low"}}"#,
    )
    .expect("bindings");
    Config::load(repo).expect("load")
}

#[test]
fn pure_dependency_parsing() {
    assert_eq!(deps("Blah. Depends on #7. More."), vec![7]);
    assert_eq!(deps("Depends on #7\nDepends on #12"), vec![7, 12]);
    assert_eq!(deps("No dependencies here"), Vec::<u64>::new());
    assert_eq!(deps("See issue #7 for context"), Vec::<u64>::new());
    assert_eq!(deps("depends on #7"), vec![7]);
}

#[test]
fn init_derives_the_slug_and_refuses_no_remote() {
    let repo = make_repo();
    let _g = prepend_stubs();
    assert!(Queue::init(&repo).is_err(), "no origin remote is refused");
    let status = std::process::Command::new("git")
        .arg("-C")
        .arg(&repo)
        .args([
            "remote",
            "add",
            "origin",
            "https://github.com/someone/thing.git",
        ])
        .status()
        .expect("git");
    assert!(status.success());
    let q = Queue::init(&repo).expect("init");
    assert_eq!(q.repo, "someone/thing");
}

#[test]
fn queue_behaviour() {
    let _g = prepend_stubs();
    let repo = make_repo();
    let status = std::process::Command::new("git")
        .arg("-C")
        .arg(&repo)
        .args([
            "remote",
            "add",
            "origin",
            "https://github.com/someone/thing.git",
        ])
        .status()
        .expect("git");
    assert!(status.success());
    let cfg = write_config(&repo);

    // Every gh call names the repository explicitly.
    let calls = repo.join("gh-calls.txt");
    set_stub(
        "gh",
        &format!(
            "echo \"$*\" >> {0}\ncase \"$*\" in\n  *\"issue list\"*) echo '[{{\"number\":1,\"title\":\"t\",\"body\":\"b\",\"labels\":[{{\"name\":\"autopilot\"}}],\"milestone\":null}}]' ;;\n  *\"issue view\"*) echo CLOSED ;;\n  *) echo '{{}}' ;;\nesac\n",
            calls.display()
        ),
    );
    let mut q = Queue::init(&repo).expect("init");
    q.load(&cfg);
    let picked = q.pick(&cfg).expect("pick");
    q.claim(picked).expect("claim");
    q.release(picked).expect("release");
    let body = std::fs::read_to_string(&calls).expect("read calls");
    let missing = body
        .lines()
        .filter(|l| !l.contains("--repo someone/thing"))
        .count();
    assert_eq!(missing, 0, "every gh call must name the repository");

    // A blocked issue is skipped in favour of its open dependency.
    set_stub(
        "gh",
        r#"case "$*" in
  *"issue list"*) cat <<JSON
[{"number":10,"title":"Blocked","body":"Depends on #11","labels":[{"name":"autopilot"}],"milestone":null},
 {"number":11,"title":"Ready","body":"No deps","labels":[{"name":"autopilot"}],"milestone":null}]
JSON
  ;;
  *"issue view"*) echo OPEN ;;
  *) echo "{}" ;;
esac"#,
    );
    let mut q = Queue::init(&repo).expect("init");
    q.load(&cfg);
    assert_eq!(q.pick(&cfg), Some(11));

    // Once the dependency closes, the blocked issue becomes eligible.
    set_stub(
        "gh",
        r#"case "$*" in
  *"issue list"*) cat <<JSON
[{"number":10,"title":"Now ready","body":"Depends on #11","labels":[{"name":"autopilot"}],"milestone":null}]
JSON
  ;;
  *"issue view"*) echo CLOSED ;;
  *) echo "{}" ;;
esac"#,
    );
    let mut q = Queue::init(&repo).expect("init");
    q.load(&cfg);
    assert_eq!(q.pick(&cfg), Some(10));

    // An unreadable dependency state keeps the dependent blocked.
    set_stub(
        "gh",
        r#"case "$*" in
  *"issue list"*) echo '[{"number":10,"title":"Blocked","body":"Depends on #11","labels":[{"name":"autopilot"}],"milestone":null}]' ;;
  *"issue view"*) exit 1 ;;
  *) echo "{}" ;;
esac"#,
    );
    let mut q = Queue::init(&repo).expect("init");
    q.load(&cfg);
    assert_eq!(q.pick(&cfg), None);

    // Excluded labels remove an issue from the queue.
    set_stub(
        "gh",
        r#"case "$*" in
  *"issue list"*) cat <<JSON
[{"number":30,"title":"Held","body":"x","labels":[{"name":"autopilot"},{"name":"blocked"}],"milestone":null},
 {"number":31,"title":"Free","body":"x","labels":[{"name":"autopilot"}],"milestone":null}]
JSON
  ;;
  *) echo "{}" ;;
esac"#,
    );
    let mut q = Queue::init(&repo).expect("init");
    q.load(&cfg);
    assert_eq!(q.pick(&cfg), Some(31));

    // Label reading uses the labels array, never the prose.
    set_stub(
        "gh",
        r#"case "$*" in
  *"issue list"*) cat <<JSON
[{"number":20,"title":"Real","body":"not needs-human","labels":[{"name":"autopilot"}],"milestone":null},
 {"number":21,"title":"Human","body":"plain","labels":[{"name":"autopilot"},{"name":"needs-human"}],"milestone":null}]
JSON
  ;;
  *) echo "{}" ;;
esac"#,
    );
    let mut q = Queue::init(&repo).expect("init");
    q.load(&cfg);
    assert!(q.has_label(21, "needs-human"));
    assert!(!q.has_label(20, "needs-human"));
    assert_eq!(q.field(20, "title"), Some("Real".into()));

    // An empty queue is a normal outcome, not an error.
    set_stub(
        "gh",
        "case \"$*\" in *\"issue list\"*) echo '[]' ;; *) echo '{}' ;; esac",
    );
    let mut q = Queue::init(&repo).expect("init");
    q.load(&cfg);
    assert_eq!(q.pick(&cfg), None);

    // A gh failure degrades to empty rather than crashing.
    set_stub("gh", "exit 1");
    let mut q = Queue::init(&repo).expect("init");
    q.load(&cfg);
    assert_eq!(q.pick(&cfg), None);
}

#[test]
fn a_missing_ready_label_is_named() {
    let _g = prepend_stubs();
    let repo = make_repo();
    let status = std::process::Command::new("git")
        .arg("-C")
        .arg(&repo)
        .args([
            "remote",
            "add",
            "origin",
            "https://github.com/someone/thing.git",
        ])
        .status()
        .expect("git");
    assert!(status.success());
    let cfg = write_config(&repo);
    let log = repo.join("queue.log");
    autopilot::log::set_file(Some(log.clone()));
    set_stub(
        "gh",
        "case \"$1 $2\" in \"issue list\") echo '[]' ;; \"label list\") printf 'bug\\nenhancement\\n' ;; *) echo '{}' ;; esac",
    );
    let mut q = Queue::init(&repo).expect("init");
    q.load(&cfg);
    let body = std::fs::read_to_string(&log).expect("log");
    assert!(
        body.contains("does not exist"),
        "names the missing label: {body}"
    );
    assert!(
        body.contains("someone/thing"),
        "names the repository: {body}"
    );
}

#[test]
fn a_genuine_gh_failure_is_reported_not_swallowed() {
    let _g = prepend_stubs();
    let repo = make_repo();
    let status = std::process::Command::new("git")
        .arg("-C")
        .arg(&repo)
        .args([
            "remote",
            "add",
            "origin",
            "https://github.com/someone/thing.git",
        ])
        .status()
        .expect("git");
    assert!(status.success());
    let cfg = write_config(&repo);
    let log = repo.join("queue.log");
    autopilot::log::set_file(Some(log.clone()));
    set_stub(
        "gh",
        "echo \"GraphQL: Could not resolve to a Repository with the name 'owner/name'. (repository)\" >&2; exit 1",
    );
    let mut q = Queue::init(&repo).expect("init");
    q.load(&cfg);
    let body = std::fs::read_to_string(&log).expect("log");
    assert!(
        body.contains("Could not resolve to a Repository"),
        "the reason reaches the log: {body}"
    );
}

#[test]
fn the_label_check_does_not_run_when_the_list_is_non_empty() {
    let _g = prepend_stubs();
    let repo = make_repo();
    let status = std::process::Command::new("git")
        .arg("-C")
        .arg(&repo)
        .args([
            "remote",
            "add",
            "origin",
            "https://github.com/someone/thing.git",
        ])
        .status()
        .expect("git");
    assert!(status.success());
    let cfg = write_config(&repo);
    let marker = repo.join("label-called");
    set_stub(
        "gh",
        &format!(
            "case \"$1 $2\" in \"issue list\") echo '[{{\"number\":1,\"title\":\"t\",\"body\":\"b\",\"labels\":[{{\"name\":\"autopilot\"}}],\"milestone\":null}}]' ;; \"label list\") echo called >> {} ;; *) echo '{{}}' ;; esac",
            marker.display()
        ),
    );
    let mut q = Queue::init(&repo).expect("init");
    q.load(&cfg);
    assert!(
        !marker.exists(),
        "the label check must stay off the busy path"
    );
}
