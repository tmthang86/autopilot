use autopilot::spawn::{run_capture, SpawnOutcome};
mod common;
use common::tmpdir;
use std::process::Command;

#[test]
fn captures_stdout_and_stderr() {
    let mut c = Command::new("sh");
    c.args(["-c", "echo out; echo err >&2"]);
    match run_capture(&mut c, b"", 10).expect("run") {
        SpawnOutcome::Exited(o) => {
            assert_eq!(o.status.code(), Some(0));
            assert_eq!(String::from_utf8_lossy(&o.stdout).trim(), "out");
            assert_eq!(String::from_utf8_lossy(&o.stderr).trim(), "err");
        }
        SpawnOutcome::TimedOut => panic!("unexpected timeout"),
    }
}

#[test]
fn a_timeout_kills_the_agents_children_too() {
    let d = tmpdir();
    let pidfile = d.join("pid");
    let script = format!(
        "sh -c 'sleep 300' & echo $! > {0}; sleep 300",
        pidfile.display()
    );
    let mut c = Command::new("sh");
    c.args(["-c", &script]);
    match run_capture(&mut c, b"", 1).expect("run") {
        SpawnOutcome::TimedOut => {}
        SpawnOutcome::Exited(o) => panic!("expected timeout, got {:?}", o.status),
    }
    std::thread::sleep(std::time::Duration::from_millis(300));
    let pid = std::fs::read_to_string(&pidfile)
        .expect("pidfile")
        .trim()
        .parse::<i32>()
        .expect("pid");
    let alive = unsafe { libc::kill(pid, 0) == 0 };
    assert!(!alive, "the forked child must die with its parent");
}

#[test]
fn a_large_unread_stdin_does_not_delay_the_timeout() {
    // Bigger than any OS pipe buffer, fed to a child that never reads it. A
    // synchronous write_all() before wait() starts its deadline would block
    // here for as long as the child lives, with no timeout active at all —
    // wait()'s clock must start regardless of how the write is going.
    let big = vec![b'x'; 4 * 1024 * 1024];
    let mut c = Command::new("sh");
    c.args(["-c", "sleep 300"]);
    let start = std::time::Instant::now();
    match run_capture(&mut c, &big, 1).expect("run") {
        SpawnOutcome::TimedOut => {}
        SpawnOutcome::Exited(o) => panic!("expected timeout, got {:?}", o.status),
    }
    assert!(
        start.elapsed().as_secs() < 5,
        "the timeout must fire near the configured deadline even with a large, unread stdin: took {:?}",
        start.elapsed()
    );
}
