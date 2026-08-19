mod common;

use autopilot::verdict::{self, VerdictError};
use common::tmpdir;

#[test]
fn a_missing_verdict_is_never_a_pass() {
    let d = tmpdir();
    assert_eq!(
        verdict::read(&d.join("tester-verdict.json")),
        Err(VerdictError::Missing)
    );
}

#[test]
fn a_malformed_verdict_is_never_a_pass() {
    let d = tmpdir();
    let p = d.join("tester-verdict.json");
    std::fs::write(&p, r#"{"verdict":"probably fine"}"#).expect("write");
    assert!(matches!(verdict::read(&p), Err(VerdictError::Malformed(_))));
}

#[test]
fn a_pass_verdict_reads_back() {
    let d = tmpdir();
    let p = d.join("tester-verdict.json");
    std::fs::write(
        &p,
        r#"{"verdict":"pass","reason":"tests pass","evidence":["a:1"]}"#,
    )
    .expect("write");
    let v = verdict::read(&p).expect("read");
    assert!(v.is_pass());
    assert_eq!(v.evidence, vec!["a:1"]);
}

#[test]
fn a_verdict_that_preexisted_is_refused() {
    let d = tmpdir();
    let dir = verdict::dir_for(&d, 5, "w-1", 0, "round-0");
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(
        dir.join("tester-verdict.json"),
        r#"{"verdict":"pass","reason":"x"}"#,
    )
    .expect("write");
    let err = verdict::prepare(&d, 5, "w-1", 0, "round-0", "tester").expect_err("must refuse");
    assert!(err.contains("already exists"), "{err}");
}
