mod common;

use autopilot::config::Config;
use autopilot::preflight;
use common::{make_repo, prepend_stubs, set_stub};
use std::path::Path;

fn write_cfg(repo: &Path, harness: &str, model: &str) -> Config {
    let ap = repo.join(".autopilot");
    std::fs::create_dir_all(&ap).expect("mkdir");
    let cfg = r#"{
            "project": {"main_branch": "main", "work_branch": "autopilot/main"},
            "queue": {"ready_label": "autopilot", "exclude_labels": [], "intent_marker": "Intent:"},
            "tiers": ["light"],
            "roles": {"implement": {"tier_offset": 0}, "test": {"tier_offset": 0}, "review": {"tier_offset": 0}},
            "pipeline": {"max_rounds": 2, "turn_timeout_s": 60, "wake_timeout_s": 600, "wake_budget_usd": 25.0, "review_lenses": []},
            "agent": {"permission_mode": "bypassPermissions", "default_tier": "light"},
            "verify": [{"name": "t", "cmd": "true"}],
            "pacing": {"daily_task_cap": 12, "max_attempts_per_issue": 2, "circuit_breaker_failures": 3},
            "autonomy": {"prepare_only_label": "needs-human"}
        }"#.to_string();
    std::fs::write(ap.join("config.json"), cfg).expect("cfg");
    std::fs::write(
        ap.join("tiers.local.json"),
        format!(r#"{{"light": {{"harness": "{harness}", "model": "{model}", "effort": "low"}}}}"#),
    )
    .expect("bindings");
    Config::load(repo).expect("load")
}

#[test]
fn an_adapter_whose_cli_errors_reports_the_error() {
    let _g = prepend_stubs();
    let repo = make_repo();
    let _ = write_cfg(&repo, "opencode", "opencode/m");
    set_stub(
        "opencode",
        "case \"$*\" in *providers*) exit 0 ;; *models*) echo 'Configuration is invalid at /x/config.json' >&2; exit 1 ;; *) exit 0 ;; esac",
    );
    let report = preflight::run(&repo);
    let opencode = report
        .harnesses
        .iter()
        .find(|h| h.name == "opencode")
        .expect("opencode");
    assert!(opencode.models.is_empty());
    assert_eq!(
        opencode.error.as_deref(),
        Some("Configuration is invalid at /x/config.json")
    );
    assert!(
        !opencode.available || !report.unresolved.is_empty(),
        "an errored adapter must not silently resolve"
    );
}

#[test]
fn what_preflight_calls_unresolved_stands_the_runner_down() {
    let _g = prepend_stubs();
    let repo = make_repo();
    let cfg = write_cfg(&repo, "opencode", "opencode/does-not-exist");
    set_stub(
        "opencode",
        "case \"$*\" in *providers*) exit 0 ;; *models*) echo 'provider  model'; echo 'opencode  reachable' ;; *) exit 0 ;; esac",
    );
    // The bound model is absent from the reachable list, so it is unresolved...
    assert_eq!(preflight::unresolved(&cfg), vec!["light"]);
    // ...and guard_tiers stands down for the same tier.
    assert!(autopilot::guard::tiers(&cfg, &["light"]).is_err());
}
