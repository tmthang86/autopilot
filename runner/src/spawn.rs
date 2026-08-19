//! The single door through which every subprocess is spawned.
//!
//! Rule: nothing in this crate starts a process except through here, and every
//! call carries a timeout. A local model that hangs must not hold the lock
//! forever, and a child must not outlive its wake — so children are started in
//! their own session and the whole process group is killed on timeout.

use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

pub enum SpawnOutcome {
    Exited(Output),
    TimedOut,
}

impl SpawnOutcome {
    pub fn status_code(&self) -> Option<i32> {
        match self {
            SpawnOutcome::Exited(o) => o.status.code(),
            SpawnOutcome::TimedOut => None,
        }
    }
}

fn begin(cmd: &mut Command, stdin: Stdio, stdout: Stdio, stderr: Stdio) -> io::Result<Child> {
    use std::os::unix::process::CommandExt;
    unsafe {
        cmd.pre_exec(|| {
            if libc::setsid() == -1 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
    cmd.stdin(stdin).stdout(stdout).stderr(stderr).spawn()
}

/// Writes stdin on its own thread instead of blocking the caller. `wait`'s
/// deadline is the only place a timeout is enforced, and it only starts after
/// this call returns — a synchronous `write_all` on a prompt bigger than the
/// OS pipe buffer, into a child slow to read it, would block here with no
/// deadline active at all (the exact hang class measured for `lms ls` on
/// 2026-08-16, just moved earlier).
fn spawn_stdin_writer(mut si: std::process::ChildStdin, data: Vec<u8>) {
    std::thread::spawn(move || {
        let _ = si.write_all(&data);
    });
}

struct Reader {
    handle: Option<std::thread::JoinHandle<Vec<u8>>>,
}

impl Reader {
    fn new<R: Read + Send + 'static>(mut r: R) -> Reader {
        Reader {
            handle: Some(std::thread::spawn(move || {
                let mut v = Vec::new();
                let _ = r.read_to_end(&mut v);
                v
            })),
        }
    }

    fn join(mut self) -> Vec<u8> {
        self.handle
            .take()
            .and_then(|h| h.join().ok())
            .unwrap_or_default()
    }
}

fn wait(child: &mut Child, timeout_s: u64) -> io::Result<bool> {
    let deadline = Instant::now() + Duration::from_secs(timeout_s);
    loop {
        if child.try_wait()?.is_some() {
            return Ok(false);
        }
        if Instant::now() >= deadline {
            let pgid = child.id() as i32;
            unsafe {
                libc::killpg(pgid, libc::SIGTERM);
            }
            std::thread::sleep(Duration::from_millis(500));
            unsafe {
                libc::killpg(pgid, libc::SIGKILL);
            }
            let _ = child.wait();
            return Ok(true);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Run a command, feeding `stdin` (usually empty) on its standard input and
/// capturing both stdout and stderr. Used by `gh`, `git`, and `verify`.
pub fn run_capture(cmd: &mut Command, stdin: &[u8], timeout_s: u64) -> io::Result<SpawnOutcome> {
    let mut child = begin(cmd, Stdio::piped(), Stdio::piped(), Stdio::piped())?;
    if let Some(si) = child.stdin.take() {
        spawn_stdin_writer(si, stdin.to_vec());
    }
    let out = child.stdout.take().map(Reader::new);
    let err = child.stderr.take().map(Reader::new);
    let timed_out = wait(&mut child, timeout_s)?;
    if timed_out {
        return Ok(SpawnOutcome::TimedOut);
    }
    let status = child.wait()?;
    let stdout = out.map(Reader::join).unwrap_or_default();
    let stderr = err.map(Reader::join).unwrap_or_default();
    Ok(SpawnOutcome::Exited(Output {
        status,
        stdout,
        stderr,
    }))
}

/// Run a command with stdout and stderr redirected to `log`, feeding `stdin`.
/// Used by the harness adapters, whose output can be large and must survive on
/// disk for classification and review.
pub fn run_to_file(
    cmd: &mut Command,
    stdin: &[u8],
    log: &Path,
    timeout_s: u64,
) -> io::Result<SpawnOutcome> {
    let file = File::create(log)?;
    let mut child = begin(
        cmd,
        Stdio::piped(),
        Stdio::from(file.try_clone()?),
        Stdio::from(file),
    )?;
    if let Some(si) = child.stdin.take() {
        spawn_stdin_writer(si, stdin.to_vec());
    }
    let timed_out = wait(&mut child, timeout_s)?;
    if timed_out {
        return Ok(SpawnOutcome::TimedOut);
    }
    let status = child.wait()?;
    Ok(SpawnOutcome::Exited(Output {
        status,
        stdout: Vec::new(),
        stderr: Vec::new(),
    }))
}
