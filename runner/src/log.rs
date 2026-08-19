//! Logging. Everything goes to stderr so that stdout stays clean for values.
//! When a log file is configured, each line is appended there too.

use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

static LOG_FILE: Mutex<Option<PathBuf>> = Mutex::new(None);

pub fn set_file(path: Option<PathBuf>) {
    if let Ok(mut f) = LOG_FILE.lock() {
        *f = path;
    }
}

pub fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn timestamp() -> String {
    let now = time::OffsetDateTime::now_utc();
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        now.year(),
        now.month() as u8,
        now.day(),
        now.hour(),
        now.minute(),
        now.second()
    )
}

pub fn date_today() -> String {
    let now = time::OffsetDateTime::now_utc();
    format!(
        "{:04}-{:02}-{:02}",
        now.year(),
        now.month() as u8,
        now.day()
    )
}

pub fn wake_id() -> String {
    format!("w-{}", unix_now())
}

pub fn info(msg: &str) {
    emit("INFO", msg);
}

pub fn warn(msg: &str) {
    emit("WARN", msg);
}

pub fn error(msg: &str) {
    emit("ERROR", msg);
}

fn emit(level: &str, msg: &str) {
    let line = format!("{} {level} {msg}", timestamp());
    eprintln!("{line}");
    if let Ok(file) = LOG_FILE.lock() {
        if let Some(path) = file.as_ref() {
            if let Ok(mut fh) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
            {
                let _ = writeln!(fh, "{line}");
            }
        }
    }
}
