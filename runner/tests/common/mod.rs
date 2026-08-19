//! Shared helpers for the integration tests.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};

static N: AtomicU64 = AtomicU64::new(0);
static ENV: Mutex<()> = Mutex::new(());

pub fn tmpdir() -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "autopilot-test-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&d).expect("create temp dir");
    d
}

pub fn write_executable(path: &Path, body: &str) {
    std::fs::write(path, body).expect("write stub");
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
}

pub fn stub_dir() -> PathBuf {
    let d = std::env::temp_dir().join(format!("autopilot-stubs-{}", std::process::id()));
    std::fs::create_dir_all(&d).expect("stub dir");
    d
}

pub fn set_stub(name: &str, script: &str) {
    write_executable(&stub_dir().join(name), script);
}

pub struct StubGuard {
    _guard: MutexGuard<'static, ()>,
}

/// Put the stub directory first on PATH for the lifetime of the returned guard.
pub fn prepend_stubs() -> StubGuard {
    let guard = ENV.lock().unwrap();
    let dir = stub_dir();
    let orig = std::env::var("PATH").unwrap_or_default();
    std::env::set_var("PATH", format!("{}:{}", dir.display(), orig));
    StubGuard { _guard: guard }
}

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .status()
        .expect("run git");
    assert!(status.success(), "git {args:?} failed");
}

pub fn make_repo() -> PathBuf {
    let d = tmpdir();
    git(&d, &["init", "-q"]);
    git(&d, &["config", "user.name", "Test"]);
    git(&d, &["config", "user.email", "test@example.com"]);
    git(&d, &["checkout", "-q", "-b", "main"]);
    std::fs::write(d.join("seed.txt"), "seed").expect("seed");
    git(&d, &["add", "-A"]);
    git(&d, &["commit", "-q", "-m", "seed"]);
    d
}
