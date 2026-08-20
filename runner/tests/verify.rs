mod common;

use autopilot::config::Config;
use autopilot::verify;
use common::make_repo;
use std::path::Path;

fn config_with_verify(repo: &Path, verify: &str) -> Config {
    let ap = repo.join(".autopilot");
    std::fs::create_dir_all(&ap).expect("mkdir");
    let cfg = format!(
        r#"{{
            "project": {{"main_branch": "main", "work_branch": "autopilot/main"}},
            "queue": {{"ready_label": "autopilot", "exclude_labels": [], "intent_marker": "Intent:"}},
            "tiers": ["light"],
            "roles": {{"implement": {{"tier_offset": 0}}, "test": {{"tier_offset": 0}}, "review": {{"tier_offset": 1}}}},
            "pipeline": {{"max_rounds": 2, "turn_timeout_s": 60, "wake_timeout_s": 600, "wake_budget_usd": 25.0, "review_lenses": []}},
            "agent": {{"permission_mode": "bypassPermissions", "default_tier": "light"}},
            "verify": {verify},
            "pacing": {{"daily_task_cap": 12, "max_attempts_per_issue": 2, "circuit_breaker_failures": 3}},
            "autonomy": {{"prepare_only_label": "needs-human"}}
        }}"#
    );
    std::fs::write(ap.join("config.json"), cfg).expect("cfg");
    std::fs::write(
        ap.join("tiers.local.json"),
        r#"{"light": {"harness": "dummy", "model": "m", "effort": "low"}}"#,
    )
    .expect("bindings");
    Config::load(repo).expect("load")
}

#[test]
fn all_commands_passing_returns_success() {
    let repo = make_repo();
    let cfg = config_with_verify(
        &repo,
        r#"[{"name":"a","cmd":"true"},{"name":"b","cmd":"true"}]"#,
    );
    assert!(verify::run(&cfg, &repo).is_ok());
}

#[test]
fn one_failing_command_fails_the_whole_gate_and_stops() {
    let repo = make_repo();
    let marker = repo.join("c.marker");
    let cmd = format!(
        r#"[{{"name":"a","cmd":"true"}},{{"name":"b","cmd":"echo boom >&2; false"}},{{"name":"c","cmd":"echo ran > {}"}}]"#,
        marker.display()
    );
    let cfg = config_with_verify(&repo, &cmd);
    let err = verify::run(&cfg, &repo).expect_err("must fail");
    assert_eq!(err.name, "b");
    assert!(err.output.contains("boom"));
    assert!(!marker.exists(), "commands after the failure must not run");
}

#[test]
fn an_empty_verify_list_is_refused() {
    let repo = make_repo();
    let cfg = config_with_verify(&repo, "[]");
    assert!(verify::run(&cfg, &repo).is_err());
}

#[test]
fn commands_run_inside_the_project_root() {
    let repo = make_repo();
    let cfg = config_with_verify(&repo, r#"[{"name":"cwd","cmd":"test -f seed.txt"}]"#);
    assert!(verify::run(&cfg, &repo).is_ok());
}

#[test]
fn a_configured_timeout_bounds_a_single_verify_command() {
    let repo = make_repo();
    let ap = repo.join(".autopilot");
    std::fs::create_dir_all(&ap).expect("mkdir");
    let cfg = r#"{
            "project": {"main_branch": "main", "work_branch": "autopilot/main"},
            "queue": {"ready_label": "autopilot", "exclude_labels": [], "intent_marker": "Intent:"},
            "tiers": ["light"],
            "roles": {"implement": {"tier_offset": 0}, "test": {"tier_offset": 0}, "review": {"tier_offset": 1}},
            "pipeline": {"max_rounds": 2, "turn_timeout_s": 60, "wake_timeout_s": 600, "wake_budget_usd": 25.0, "review_lenses": [], "verify_timeout_s": 1},
            "agent": {"permission_mode": "bypassPermissions", "default_tier": "light"},
            "verify": [{"name":"slow","cmd":"sleep 5"}],
            "pacing": {"daily_task_cap": 12, "max_attempts_per_issue": 2, "circuit_breaker_failures": 3},
            "autonomy": {"prepare_only_label": "needs-human"}
        }"#;
    std::fs::write(ap.join("config.json"), cfg).expect("cfg");
    std::fs::write(
        ap.join("tiers.local.json"),
        r#"{"light": {"harness": "dummy", "model": "m", "effort": "low"}}"#,
    )
    .expect("bindings");
    let cfg = Config::load(&repo).expect("load");
    let start = std::time::Instant::now();
    let err = verify::run(&cfg, &repo).expect_err("a sleep past the timeout must fail");
    assert!(
        start.elapsed().as_secs() < 4,
        "killed by the configured timeout, not by the command finishing"
    );
    assert!(err.output.contains("timed out"));
}
