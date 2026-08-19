use autopilot::config::{Config, ConfigError};
mod common;
use common::tmpdir;
use autopilot::tier;
use std::path::Path;

fn write(project: &Path, cfg: &str, bindings: &str) {
    let ap = project.join(".autopilot");
    std::fs::create_dir_all(&ap).expect("mkdir");
    std::fs::write(ap.join("config.json"), cfg).expect("write cfg");
    if !bindings.is_empty() {
        std::fs::write(ap.join("tiers.local.json"), bindings).expect("write bindings");
    }
}

const CFG: &str = r#"{
    "project": {"name": "demo", "main_branch": "main", "work_branch": "autopilot/main"},
    "queue": {"ready_label": "autopilot", "exclude_labels": ["blocked"], "intent_marker": "Intent:"},
    "tiers": ["light", "standard", "deep"],
    "roles": {"implement": {"tier_offset": 0}, "test": {"tier_offset": 0}, "review": {"tier_offset": 1}},
    "pipeline": {"max_rounds": 2, "turn_timeout_s": 60, "wake_timeout_s": 600, "wake_budget_usd": 25.0, "review_lenses": ["plan-conformance"]},
    "agent": {"permission_mode": "bypassPermissions", "default_tier": "standard"},
    "verify": [{"name": "t", "cmd": "true"}],
    "pacing": {"daily_task_cap": 12, "max_attempts_per_issue": 2, "circuit_breaker_failures": 3},
    "autonomy": {"prepare_only_label": "needs-human"}
}"#;

const BINDINGS: &str = r#"{
    "light": {"harness": "opencode", "model": "opencode/m", "effort": "low", "budget_usd": 0.0},
    "standard": {"harness": "claude", "model": "sonnet", "effort": "low", "budget_usd": 5.0},
    "deep": {"harness": "claude", "model": "opus", "effort": "high", "budget_usd": 15.0}
}"#;

#[test]
fn loads_two_layers_together() {
    let d = tmpdir();
    write(&d, CFG, BINDINGS);
    let c = Config::load(&d).expect("load");
    assert_eq!(c.tiers.len(), 3);
    assert_eq!(c.bindings["standard"].harness, "claude");
    assert_eq!(c.queue.tier_label_prefix, "tier:");
}

#[test]
fn a_tier_with_no_binding_is_refused_by_name() {
    let d = tmpdir();
    write(&d, CFG, r#"{"light": {"harness": "x", "model": "m", "effort": "l"}}"#);
    match Config::load(&d) {
        Err(ConfigError::UnboundTier(t)) => assert_eq!(t, "standard"),
        other => panic!("expected UnboundTier(standard), got {other:?}"),
    }
}

#[test]
fn an_old_config_fails_loudly_rather_than_running_wrong() {
    let d = tmpdir();
    let cfg = CFG.replace(
        r#""agent": {"permission_mode": "bypassPermissions", "default_tier": "standard"}"#,
        r#""agent": {"permission_mode": "bypassPermissions", "default_model": "sonnet"}"#,
    );
    write(&d, &cfg, BINDINGS);
    let e = Config::load(&d).expect_err("must not load");
    assert!(format!("{e}").contains("default_model"), "must name the key: {e}");
}

#[test]
fn a_stored_zero_is_not_confused_with_absent() {
    let d = tmpdir();
    write(
        &d,
        CFG,
        r#"{"light": {"harness": "pi", "model": "deepseek/m", "effort": "low", "budget_usd": 0.0},
            "standard": {"harness": "claude", "model": "sonnet", "effort": "low"},
            "deep": {"harness": "claude", "model": "opus", "effort": "high"}}"#,
    );
    let c = Config::load(&d).expect("load");
    assert_eq!(c.bindings["light"].budget_usd, 0.0);
}

#[test]
fn a_missing_bindings_file_is_an_unbound_tier() {
    let d = tmpdir();
    write(&d, CFG, "");
    assert!(matches!(Config::load(&d), Err(ConfigError::UnboundTier(_))));
}

fn cfg(path: &Path, tiers: &[&str]) -> Config {
    let bindings: serde_json::Map<String, serde_json::Value> = tiers
        .iter()
        .map(|t| {
            (
                t.to_string(),
                serde_json::json!({"harness":"claude","model":"m","effort":"low","budget_usd":0.0}),
            )
        })
        .collect();
    let c = serde_json::json!({
        "project": {"main_branch":"main","work_branch":"autopilot/main"},
        "queue": {"ready_label":"autopilot","exclude_labels":[],"intent_marker":"Intent:"},
        "tiers": tiers,
        "roles": {"implement":{"tier_offset":0},"test":{"tier_offset":0},"review":{"tier_offset":1}},
        "pipeline": {"max_rounds":2,"turn_timeout_s":60,"wake_timeout_s":600,"wake_budget_usd":25.0,"review_lenses":[]},
        "agent": {"permission_mode":"bypassPermissions","default_tier":"standard"},
        "verify": [{"name":"t","cmd":"true"}],
        "pacing": {"daily_task_cap":12,"max_attempts_per_issue":2,"circuit_breaker_failures":3},
        "autonomy": {"prepare_only_label":"needs-human"}
    });
    let ap = path.join(".autopilot");
    std::fs::create_dir_all(&ap).expect("mkdir");
    std::fs::write(ap.join("config.json"), c.to_string()).expect("write");
    std::fs::write(
        ap.join("tiers.local.json"),
        serde_json::to_string(&bindings).expect("json"),
    )
    .expect("write");
    Config::load(path).expect("load")
}

#[test]
fn the_top_tier_offset_by_one_is_itself() {
    let d = tmpdir();
    let c = cfg(&d, &["light", "standard", "deep"]);
    let (name, _) = tier::resolve(&c, "deep", 1).expect("resolves");
    assert_eq!(name, "deep", "the ladder has a ceiling and must not overflow");
}

#[test]
fn the_bottom_tier_offset_down_is_itself() {
    let d = tmpdir();
    let c = cfg(&d, &["light", "deep"]);
    let (name, _) = tier::resolve(&c, "light", -1).expect("resolves");
    assert_eq!(name, "light");
}

#[test]
fn an_unknown_tier_name_is_an_error_not_the_default() {
    let d = tmpdir();
    let c = cfg(&d, &["light", "deep"]);
    assert!(tier::resolve(&c, "medium", 0).is_err());
}

#[test]
fn one_step_up_moves_along_the_list() {
    let d = tmpdir();
    let c = cfg(&d, &["light", "standard", "deep"]);
    let (name, _) = tier::resolve(&c, "standard", 1).expect("resolves");
    assert_eq!(name, "deep");
}
