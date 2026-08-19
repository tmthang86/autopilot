mod common;

use autopilot::run_once;
use common::{make_repo, prepend_stubs, set_stub};
use std::path::Path;
use std::process::ExitCode;

fn write_project(repo: &Path, verify: &str, lenses: &str) {
    let ap = repo.join(".autopilot");
    std::fs::create_dir_all(ap.join("logs")).expect("mkdir");
    let cfg = format!(
        r#"{{
            "project": {{"name": "demo", "main_branch": "main", "work_branch": "autopilot/main"}},
            "queue": {{"ready_label": "autopilot", "exclude_labels": ["blocked","needs-human","status:in-progress"], "intent_marker": "Intent:", "tier_label_prefix": "tier:"}},
            "tiers": ["light"],
            "roles": {{"implement": {{"tier_offset": 0}}, "test": {{"tier_offset": 0}}, "review": {{"tier_offset": 0}}}},
            "pipeline": {{"max_rounds": 2, "turn_timeout_s": 60, "wake_timeout_s": 600, "wake_budget_usd": 25.0, "review_lenses": {lenses}}},
            "agent": {{"permission_mode": "bypassPermissions", "default_tier": "light"}},
            "verify": {verify},
            "pacing": {{"daily_task_cap": 12, "max_attempts_per_issue": 2, "circuit_breaker_failures": 3}},
            "autonomy": {{"prepare_only_label": "needs-human"}}
        }}"#
    );
    std::fs::write(ap.join("config.json"), cfg).expect("cfg");
    std::fs::write(
        ap.join("tiers.local.json"),
        r#"{"light": {"harness": "claude", "model": "sonnet", "effort": "low", "budget_usd": 0.0}}"#,
    )
    .expect("bindings");
}

fn setup_repo() -> std::path::PathBuf {
    let repo = make_repo();
    let remotedir = common::tmpdir();
    let bare = remotedir.join("remote.git");
    std::process::Command::new("git")
        .arg("init")
        .arg("--bare")
        .arg("-q")
        .arg(&bare)
        .status()
        .expect("bare");
    let mut c = std::process::Command::new("git");
    c.arg("-C")
        .arg(&repo)
        .args(["remote", "add", "origin"])
        .arg(&bare);
    assert!(c.status().expect("remote").success());

    std::fs::create_dir_all(repo.join("docs/plans")).expect("mkdir");
    std::fs::write(repo.join("docs/plans/0001.md"), "the plan").expect("plan");
    let mut c = std::process::Command::new("git");
    c.arg("-C").arg(&repo).args(["add", "-A"]);
    assert!(c.status().expect("add").success());
    let mut c = std::process::Command::new("git");
    c.arg("-C").arg(&repo).args(["commit", "-q", "-m", "plan"]);
    assert!(c.status().expect("commit").success());
    let mut c = std::process::Command::new("git");
    c.arg("-C")
        .arg(&repo)
        .args(["checkout", "-q", "-b", "autopilot/main"]);
    assert!(c.status().expect("branch").success());
    let mut c = std::process::Command::new("git");
    c.arg("-C").arg(&repo).args(["checkout", "-q", "main"]);
    assert!(c.status().expect("checkout").success());
    repo
}

const GH_HAPPY: &str = r#"echo "$*" >> "$GH_CALLS"
case "$1 $2" in
  "issue list") cat <<'JSON'
[{"number":5,"title":"Do a thing","body":"Intent: docs/plans/0001.md\nCreate marker.txt","labels":[{"name":"autopilot"},{"name":"tier:light"}],"milestone":null}]
JSON
  ;;
  *) echo "{}" ;;
esac
exit 0"#;

const CLAUDE_HAPPY: &str = r#"case "$*" in *--version*) exit 0 ;; *" -p ok"*) exit 0 ;; esac
cat > "$CLAUDE_STDIN"
vp=$(grep -oE '[^ "]+-verdict\.json' "$CLAUDE_STDIN" | head -1)
if grep -q 'You are the TESTER' "$CLAUDE_STDIN"; then
    printf '{"verdict":"pass","reason":"tests pass","evidence":[]}' > "$vp"
    echo '{"type":"result","is_error":false}'
elif grep -q 'You are the REVIEWER' "$CLAUDE_STDIN"; then
    printf '{"verdict":"pass","reason":"lens satisfied","evidence":[]}' > "$vp"
    echo '{"type":"result","is_error":false}'
else
    echo done > marker.txt
    echo '{"type":"result","is_error":false}'
fi
exit 0"#;

#[test]
fn a_full_three_role_run_merges_and_closes() {
    let _g = prepend_stubs();
    let repo = setup_repo();
    write_project(&repo, r#"[{"name":"t","cmd":"true"}]"#, "[]");
    std::env::set_var("GH_CALLS", repo.join("gh-calls.txt"));
    std::env::set_var("CLAUDE_STDIN", repo.join("stdin.txt"));
    set_stub("gh", GH_HAPPY);
    set_stub("claude", CLAUDE_HAPPY);

    assert_eq!(run_once::run(&repo), ExitCode::SUCCESS);

    let calls = std::fs::read_to_string(repo.join("gh-calls.txt")).expect("calls");
    assert!(
        calls.contains("issue close 5"),
        "the issue is closed: {calls}"
    );
    let log = std::process::Command::new("git")
        .arg("-C")
        .arg(&repo)
        .args(["log", "-1", "--pretty=%B"])
        .output()
        .expect("log");
    assert!(
        String::from_utf8_lossy(&log.stdout).contains("Closes #5"),
        "the work carries the closing trailer"
    );
    let journal = std::fs::read_to_string(repo.join(".autopilot/journal.jsonl")).expect("journal");
    let starts = journal
        .lines()
        .filter(|l| l.contains("\"role_start\""))
        .count();
    let ends = journal
        .lines()
        .filter(|l| l.contains("\"role_end\""))
        .count();
    assert_eq!(starts, ends, "no orphaned roles: {journal}");
    assert!(
        journal.contains("\"merged\""),
        "the wake ended merged: {journal}"
    );
}

#[test]
fn a_red_verify_discards_the_work() {
    let _g = prepend_stubs();
    let repo = setup_repo();
    // The first verify (after the implementer) passes; the final verify, after
    // the rebase, fails. A red final verify must rewind to START_SHA.
    write_project(
        &repo,
        r#"[{"name":"counted","cmd":"if [ -f .vcount ]; then exit 1; else touch .vcount; fi"}]"#,
        "[]",
    );
    std::env::set_var("GH_CALLS", repo.join("gh-calls.txt"));
    std::env::set_var("CLAUDE_STDIN", repo.join("stdin.txt"));
    set_stub("gh", GH_HAPPY);
    set_stub(
        "claude",
        r#"case "$*" in *--version*) exit 0 ;; *" -p ok"*) exit 0 ;; esac
echo '{"type":"result","is_error":false}'; echo junk > should-not-land.txt; exit 0"#,
    );

    let before = std::process::Command::new("git")
        .arg("-C")
        .arg(&repo)
        .args(["rev-parse", "HEAD"])
        .output()
        .expect("sha");
    assert_eq!(run_once::run(&repo), ExitCode::SUCCESS);
    let after = std::process::Command::new("git")
        .arg("-C")
        .arg(&repo)
        .args(["rev-parse", "HEAD"])
        .output()
        .expect("sha");
    assert_eq!(
        before.stdout, after.stdout,
        "a red verify creates no commit"
    );
    assert!(!repo.join("should-not-land.txt").exists());
    let journal = std::fs::read_to_string(repo.join(".autopilot/journal.jsonl")).expect("journal");
    assert!(journal.contains("\"failed\""), "the wake ended failed");
}

#[test]
fn usage_exhaustion_consumes_no_retry() {
    let _g = prepend_stubs();
    let repo = setup_repo();
    write_project(&repo, r#"[{"name":"t","cmd":"true"}]"#, "[]");
    std::env::set_var("GH_CALLS", repo.join("gh-calls.txt"));
    std::env::set_var("CLAUDE_STDIN", repo.join("stdin.txt"));
    set_stub("gh", GH_HAPPY);
    set_stub(
        "claude",
        "case \"$*\" in *--version*) exit 0 ;; esac\nexit 1",
    );

    assert_eq!(run_once::run(&repo), ExitCode::SUCCESS);
    let state = std::fs::read_to_string(repo.join(".autopilot/state.json")).expect("state");
    let v: serde_json::Value = serde_json::from_str(&state).expect("json");
    assert_eq!(
        v["consecutive_failures"], 0,
        "exhaustion is not a task failure"
    );
    assert!(
        v["resume_after"].as_i64().expect("num") > 0,
        "resume_after is set into the future"
    );
    let journal = std::fs::read_to_string(repo.join(".autopilot/journal.jsonl")).expect("journal");
    assert!(journal.contains("provider_unavailable"));
}
