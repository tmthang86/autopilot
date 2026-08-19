use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static N: AtomicU64 = AtomicU64::new(0);

pub fn tmpdir() -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "autopilot-test-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&d).expect("create temp dir");
    d
}
