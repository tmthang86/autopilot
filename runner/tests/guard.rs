mod common;

use autopilot::config::Config;
use autopilot::guard::{self, Lock};
use autopilot::state::State;
use common::{make_repo, tmpdir};
use std::path::Path;

fn config_with(repo: &Path, quiet_hours: &str) -> Config {
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
            "verify": [{{"name": "t", "cmd": "true"}}],
            "pacing": {{"daily_task_cap": 12, "quiet_hours": {quiet_hours}, "max_attempts_per_issue": 2, "circuit_breaker_failures": 3}},
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
fn stop_file_halts_the_run() {
    let repo = make_repo();
    let ap = repo.join(".autopilot");
    std::fs::create_dir_all(&ap).expect("mkdir");
    assert!(guard::stop_file(&ap).is_ok());
    std::fs::write(ap.join("STOP"), "").expect("stop");
    assert!(guard::stop_file(&ap).is_err());
}

#[test]
fn the_lock_is_exclusive_and_released_on_drop() {
    let repo = make_repo();
    let ap = repo.join(".autopilot");
    std::fs::create_dir_all(&ap).expect("mkdir");
    let l = Lock::acquire(&ap).expect("first");
    assert!(Lock::acquire(&ap).is_err(), "second acquisition refused");
    drop(l);
    assert!(Lock::acquire(&ap).is_ok(), "reacquirable after release");
}

#[test]
fn a_stale_lock_is_broken() {
    let repo = make_repo();
    let ap = repo.join(".autopilot");
    std::fs::create_dir_all(ap.join("lock")).expect("mkdir");
    std::fs::write(ap.join("lock/pid"), "999999").expect("pid");
    assert!(
        Lock::acquire(&ap).is_ok(),
        "a dead pid must not wedge the loop"
    );
}

#[test]
fn the_lock_is_released_even_when_the_run_panics() {
    let repo = make_repo();
    let ap = repo.join(".autopilot");
    std::fs::create_dir_all(&ap).expect("mkdir");
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _l = Lock::acquire(&ap).expect("lock");
        panic!("boom");
    }));
    assert!(
        Lock::acquire(&ap).is_ok(),
        "a leaked lock wedges every later wake"
    );
}

#[test]
fn resume_after_halts_until_the_window_opens() {
    let repo = make_repo();
    let ap = repo.join(".autopilot");
    std::fs::create_dir_all(&ap).expect("mkdir");
    let mut s = State::init(&ap.join("state.json")).expect("state");
    assert!(guard::resume_after(&s).is_ok());
    s.set_num("resume_after", autopilot::log::unix_now() as i64 + 3600);
    assert!(guard::resume_after(&s).is_err());
}

#[test]
fn the_daily_cap_halts_and_rolls_over() {
    let repo = make_repo();
    let cfg = config_with(&repo, "[]");
    let mut s = State::init(&repo.join(".autopilot/state.json")).expect("state");
    s.set_str("tasks_today_date", &autopilot::log::date_today());
    s.set_num("tasks_today", 11);
    assert!(guard::daily_cap(&cfg, &mut s).is_ok());
    s.set_num("tasks_today", 12);
    assert!(guard::daily_cap(&cfg, &mut s).is_err());
    s.set_str("tasks_today_date", "2020-01-01");
    assert!(guard::daily_cap(&cfg, &mut s).is_ok());
    assert_eq!(
        s.get_num("tasks_today", -1),
        0,
        "rollover zeroes the counter"
    );
}

fn allow(cfg: &Config, hm: &str) -> bool {
    guard::quiet_hours(cfg, hm).is_ok()
}

#[test]
fn quiet_hours_boundaries() {
    let repo = make_repo();
    let none = config_with(&repo, "[]");
    assert!(allow(&none, "1200"));

    let day = config_with(&repo, r#"["09:00-18:00"]"#);
    assert!(!allow(&day, "1200"));
    assert!(!allow(&day, "0900"));
    assert!(!allow(&day, "1800"));
    assert!(allow(&day, "0859"));
    assert!(allow(&day, "1801"));
    assert!(allow(&day, "0300"));

    let wrap = config_with(&repo, r#"["23:00-07:00"]"#);
    assert!(!allow(&wrap, "2330"));
    assert!(!allow(&wrap, "0200"));
    assert!(!allow(&wrap, "0700"));
    assert!(allow(&wrap, "1200"));
    assert!(allow(&wrap, "2259"));

    let octal = config_with(&repo, r#"["08:00-09:00"]"#);
    assert!(!allow(&octal, "0830"), "08 and 09 must not read as octal");
}

#[test]
fn project_present_checks_the_git_directory() {
    let repo = make_repo();
    assert!(guard::project_present(&repo).is_ok());
    let missing = tmpdir();
    assert!(guard::project_present(&missing).is_err());
}
