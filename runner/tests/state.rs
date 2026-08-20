use autopilot::state::State;
mod common;
use common::tmpdir;

#[test]
fn init_writes_defaults() {
    let d = tmpdir();
    let s = State::init(&d.join("state.json")).expect("init");
    assert_eq!(s.get_num("resume_after", -1), 0);
}

#[test]
fn numbers_and_strings_round_trip() {
    let d = tmpdir();
    let mut s = State::init(&d.join("state.json")).expect("init");
    s.set_num("resume_after", 1755000000);
    s.set_str("tasks_today_date", "2026-08-14");
    s.save().expect("save");

    let s2 = State::init(&d.join("state.json")).expect("re-init");
    assert_eq!(s2.get_num("resume_after", -1), 1755000000);
    assert_eq!(s2.get_str("tasks_today_date", "x"), "2026-08-14");
}

#[test]
fn a_stored_zero_is_not_confused_with_absent() {
    let d = tmpdir();
    let mut s = State::init(&d.join("state.json")).expect("init");
    s.set_num("backoff_step", 0);
    s.save().expect("save");
    let s2 = State::init(&d.join("state.json")).expect("re-init");
    assert_eq!(s2.get_num("backoff_step", 99), 0);
}

#[test]
fn corrupt_state_is_rebuilt_from_defaults() {
    let d = tmpdir();
    let f = d.join("state.json");
    std::fs::write(&f, "garbage").expect("write");
    let s = State::init(&f).expect("init");
    assert_eq!(s.get_num("tasks_today", -1), 0);
}

#[test]
fn init_creates_missing_parent_directories() {
    let d = tmpdir();
    let f = d.join("nested/dir/state.json");
    let s = State::init(&f).expect("init");
    assert_eq!(s.get_num("tasks_today", -1), 0);
}

#[test]
fn attempts_are_counted_and_evicted() {
    let d = tmpdir();
    let mut s = State::init(&d.join("state.json")).expect("init");
    assert_eq!(s.record_attempt(7), 1);
    assert_eq!(s.record_attempt(7), 2);
    assert_eq!(s.record_attempt(8), 1);
    s.prune_attempts(&[7]);
    assert_eq!(s.attempt_count(7), 2, "an open issue keeps its attempts");
    assert_eq!(
        s.attempt_count(8),
        0,
        "a closed issue's attempts are evicted"
    );
    s.clear_attempt(7);
    assert_eq!(s.attempt_count(7), 0);
}

#[test]
fn a_non_object_attempts_field_does_not_panic() {
    // A crash or a hand edit can leave a valid-JSON `attempts` that is not an
    // object. record_attempt must rebuild it, not panic mid-run (Rule Zero).
    let d = tmpdir();
    let f = d.join("state.json");
    std::fs::write(&f, r#"{"attempts": 3}"#).expect("write");
    let mut s = State::init(&f).expect("init");
    assert_eq!(
        s.record_attempt(5),
        1,
        "counting restarts from an empty map"
    );
    assert_eq!(s.attempt_count(5), 1);
    s.save().expect("save");
    let s2 = State::init(&f).expect("re-init");
    assert_eq!(s2.attempt_count(5), 1, "the rebuilt map round-trips");
}
