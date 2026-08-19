use autopilot::journal::{CostSource, Event, Journal};
mod common;
use common::tmpdir;
use std::fs;

#[test]
fn a_role_end_records_its_cost_source() {
    let dir = tmpdir();
    let mut j = Journal::open(&dir, 1 << 20).expect("open");
    j.append(Event::RoleEnd {
        role: "implement".into(),
        round: 0,
        tier: "standard".into(),
        classify: "ok".into(),
        cost_usd: Some(0.42),
        cost_source: CostSource::Reported,
        duration_s: 312,
        lens: None,
    })
    .expect("append");
    let body = fs::read_to_string(dir.join(".autopilot/journal.jsonl")).expect("read");
    let v: serde_json::Value = serde_json::from_str(body.trim()).expect("valid JSONL");
    assert_eq!(v["event"], "role_end");
    assert_eq!(v["cost_source"], "reported");
    assert_eq!(v["tier"], "standard");
}

#[test]
fn an_unknown_cost_is_never_written_as_zero() {
    let dir = tmpdir();
    let mut j = Journal::open(&dir, 1 << 20).expect("open");
    j.append(Event::RoleEnd {
        role: "test".into(),
        round: 0,
        tier: "light".into(),
        classify: "ok".into(),
        cost_usd: None,
        cost_source: CostSource::Unknown,
        duration_s: 5,
        lens: None,
    })
    .expect("append");
    let body = fs::read_to_string(dir.join(".autopilot/journal.jsonl")).expect("read");
    let v: serde_json::Value = serde_json::from_str(body.trim()).expect("valid");
    assert!(v["cost_usd"].is_null(), "unknown cost must be null, not 0");
    assert_eq!(v["cost_source"], "unknown");
}

#[test]
fn a_role_start_with_no_role_end_is_detectable() {
    let dir = tmpdir();
    let mut j = Journal::open(&dir, 1 << 20).expect("open");
    j.append(Event::RoleStart {
        role: "review".into(),
        round: 1,
        tier: "deep".into(),
        harness: "claude".into(),
        model: "opus".into(),
        lens: Some("partial-failure".into()),
    })
    .expect("append");
    let body = fs::read_to_string(dir.join(".autopilot/journal.jsonl")).expect("read");
    let starts = body.lines().filter(|l| l.contains("\"role_start\"")).count();
    let ends = body.lines().filter(|l| l.contains("\"role_end\"")).count();
    assert_eq!((starts, ends), (1, 0), "an orphan must be start-without-end");
}

#[test]
fn a_wake_start_carries_a_nullable_issue() {
    let dir = tmpdir();
    let mut j = Journal::open(&dir, 1 << 20).expect("open");
    j.append(Event::WakeStart {
        issue: None,
        tier: None,
    })
    .expect("append");
    let body = fs::read_to_string(dir.join(".autopilot/journal.jsonl")).expect("read");
    let v: serde_json::Value = serde_json::from_str(body.trim()).expect("valid");
    assert!(v["issue"].is_null(), "a stand-down wake has no issue");
}

#[test]
fn a_full_journal_rolls_without_deleting() {
    let dir = tmpdir();
    let mut j = Journal::open(&dir, 10).expect("open");
    j.append(Event::Verify {
        result: "green".into(),
        failed: None,
    })
    .expect("append");
    drop(j);
    let _ = Journal::open(&dir, 10).expect("reopen");
    let entries: Vec<_> = fs::read_dir(dir.join(".autopilot"))
        .expect("read dir")
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with("journal-"))
        .collect();
    assert_eq!(entries.len(), 1, "the old journal was rolled, not deleted");
}
