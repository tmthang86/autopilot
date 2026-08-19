mod common;

use autopilot::harness::{self, Classification, RunOutcome, RunParams};
use autopilot::journal::CostSource;
use common::{prepend_stubs, set_stub, tmpdir};
use std::path::{Path, PathBuf};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../tests/fixtures")
        .join(name)
}

fn outcome(log: &Path, status: Option<i32>) -> RunOutcome {
    RunOutcome {
        status,
        log: log.to_path_buf(),
        cost_usd: None,
        cost_source: CostSource::Unknown,
        duration_s: 0,
        model: "sonnet".into(),
    }
}

#[test]
fn claude_only_the_final_result_event_decides() {
    let d = tmpdir();
    let log = d.join("run.jsonl");
    std::fs::write(
        &log,
        "{\"type\":\"tool_result\",\"is_error\":true}\n{\"type\":\"result\",\"is_error\":false,\"subtype\":\"success\",\"total_cost_usd\":0.05}\n",
    )
    .expect("write");
    let h = harness::by_name("claude").expect("claude");
    assert_eq!(h.classify(&outcome(&log, Some(0))), Classification::Ok);
}

#[test]
fn claude_is_error_false_is_not_treated_as_absent() {
    let d = tmpdir();
    let log = d.join("run.jsonl");
    std::fs::write(
        &log,
        "{\"type\":\"result\",\"is_error\":false,\"total_cost_usd\":0.01}\n",
    )
    .expect("write");
    let h = harness::by_name("claude").expect("claude");
    assert_eq!(h.classify(&outcome(&log, Some(0))), Classification::Ok);
}

#[test]
fn claude_a_throttled_rate_limit_event_is_provider_unavailable() {
    let d = tmpdir();
    let log = d.join("run.jsonl");
    std::fs::write(
        &log,
        "{\"type\":\"rate_limit_event\",\"status\":\"throttled\"}\n",
    )
    .expect("write");
    let h = harness::by_name("claude").expect("claude");
    assert_eq!(
        h.classify(&outcome(&log, Some(1))),
        Classification::ProviderUnavailable
    );
}

#[test]
fn claude_reports_cost_from_the_final_event() {
    let _g = prepend_stubs();
    let src = fixture("claude-result-success.json");
    set_stub("claude", &format!("cat '{}'", src.display()));
    let d = tmpdir();
    let log = d.join("run.jsonl");
    let h = harness::by_name("claude").expect("claude");
    let params = RunParams {
        model: "sonnet",
        effort: "low",
        budget_usd: 5.0,
        timeout_s: 60,
        permission_mode: "bypassPermissions",
    };
    let out = h.run("hi", &d, &log, &params).expect("run");
    assert_eq!(out.cost_usd, Some(0.0556587));
    assert_eq!(out.cost_source, CostSource::Reported);
    assert_eq!(h.classify(&out), Classification::Ok);
}

#[test]
fn pi_reads_errors_from_the_stream_not_the_exit_code() {
    let _g = prepend_stubs();
    // pi exits 0 even when the model errors; classification must read
    // stopReason from the recorded stream (measured 2026-08-19).
    let log = fixture("pi-run-error.jsonl");
    let h = harness::by_name("pi").expect("pi");
    set_stub(
        "pi",
        "case \"$*\" in *\"-p\"*) exit 0 ;; *\"--list-models\"*) echo 'provider  model'; echo 'deepseek  deepseek-v4-flash' ;; *) echo '{\"status\":\"ready\"}' ;; esac",
    );
    let o = RunOutcome {
        status: Some(0),
        log,
        cost_usd: None,
        cost_source: CostSource::Reported,
        duration_s: 0,
        model: "deepseek/deepseek-v4-flash".into(),
    };
    assert_eq!(h.classify(&o), Classification::TaskFailure);
}

#[test]
fn pi_a_clean_stream_is_ok() {
    let _g = prepend_stubs();
    let log = fixture("pi-run-success.jsonl");
    let h = harness::by_name("pi").expect("pi");
    let o = RunOutcome {
        status: Some(0),
        log,
        cost_usd: Some(0.00021028),
        cost_source: CostSource::Reported,
        duration_s: 0,
        model: "deepseek/deepseek-v4-flash".into(),
    };
    assert_eq!(h.classify(&o), Classification::Ok);
}

#[test]
fn pi_readiness_is_read_from_json_never_from_the_exit_code() {
    let _g = prepend_stubs();
    set_stub(
        "pi",
        "case \"$*\" in *auth*) echo '{\"status\":\"not_ready\",\"provider\":\"anthropic\",\"reason\":\"credentials_not_configured\"}' ; exit 0 ;; *\"--list-models\"*) echo 'provider  model' ;; *) echo '{}' ;; esac",
    );
    let h = harness::by_name("pi").expect("pi");
    let check = h.check(Some("anthropic/claude-opus"));
    assert!(
        !check.available,
        "exit 0 with a not_ready payload must not report ready"
    );
    assert!(check.error.is_some());
}

#[test]
fn opencode_reports_its_config_error_verbatim() {
    let _g = prepend_stubs();
    set_stub(
        "opencode",
        "case \"$*\" in *providers*) exit 0 ;; *models*) echo 'Configuration is invalid at /Users/x/.config/opencode/config.json' >&2 ; exit 1 ;; *) exit 0 ;; esac",
    );
    let h = harness::by_name("opencode").expect("opencode");
    let check = h.check(None);
    assert!(
        check.available,
        "providers list succeeded, so credentials exist"
    );
    assert!(check.models.is_empty());
    assert_eq!(
        check.error.as_deref(),
        Some("Configuration is invalid at /Users/x/.config/opencode/config.json")
    );
    assert!(!check.proven);
}

#[test]
fn the_default_classifier_distinguishes_a_dead_provider() {
    let _g = prepend_stubs();
    // opencode uses the default classifier: nonzero exit plus a live probe is a
    // task failure; nonzero exit plus a dead probe is ProviderUnavailable.
    let h = harness::by_name("opencode").expect("opencode");
    let o = outcome(&tmpdir().join("missing.jsonl"), Some(1));
    set_stub("opencode", "exit 0");
    assert_eq!(h.classify(&o), Classification::TaskFailure);
    set_stub("opencode", "exit 1");
    assert_eq!(h.classify(&o), Classification::ProviderUnavailable);
}
