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
