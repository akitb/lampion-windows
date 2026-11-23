//! Running programs without a console window.

use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

/// No console window for child processes.
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;
/// The child does not die with Lampion (servers keep running after the app quits).
pub const DETACHED_PROCESS: u32 = 0x0000_0008;
pub const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;

/// A `Command` that shows no console window.
pub fn command(program: impl AsRef<Path>) -> Command {
    let mut c = Command::new(program.as_ref());
    #[cfg(windows)]
    c.creation_flags(CREATE_NO_WINDOW);
    c
}

#[derive(Debug, Clone)]
pub struct Output {
    /// Exit code; -1 when the program could not start or timed out.
    pub status: i32,
    /// stdout + stderr, lossily decoded.
    pub out: String,
}

impl Output {
    pub fn ok(&self) -> bool { self.status == 0 }
}

/// Runs a program to completion and returns its exit code and combined output.
pub fn run(program: impl AsRef<Path>, args: &[&str]) -> Output {
    run_timeout(program, args, Duration::from_secs(120))
}

/// Like `run`, but kills the program after `timeout`.
pub fn run_timeout(program: impl AsRef<Path>, args: &[&str], timeout: Duration) -> Output {
    let mut cmd = command(program);
    cmd.args(args);
    output(cmd, timeout)
}

/// Runs a `command()` set up by the caller (e.g. with an environment variable) like `run_timeout`.
pub fn output(mut cmd: Command, timeout: Duration) -> Output {
    cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => return Output { status: -1, out: e.to_string() },
    };
    // Read pipes on threads so a chatty program cannot block on a full pipe.
    let mut so = child.stdout.take().unwrap();
    let mut se = child.stderr.take().unwrap();
    let t1 = std::thread::spawn(move || { let mut b = Vec::new(); let _ = std::io::Read::read_to_end(&mut so, &mut b); b });
    let t2 = std::thread::spawn(move || { let mut b = Vec::new(); let _ = std::io::Read::read_to_end(&mut se, &mut b); b });
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s.code().unwrap_or(-1),
            Ok(None) if start.elapsed() > timeout => {
                let _ = child.kill();
                let _ = child.wait();
                break -1;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(_) => break -1,
        }
    };
    let mut out = String::from_utf8_lossy(&t1.join().unwrap_or_default()).into_owned();
    out.push_str(&String::from_utf8_lossy(&t2.join().unwrap_or_default()));
    Output { status, out }
}

/// Starts a program in the background, detached from Lampion and without a window. Returns its PID.
pub fn spawn_detached(program: impl AsRef<Path>, args: &[&str], cwd: Option<&Path>) -> std::io::Result<u32> {
    let mut cmd = Command::new(program.as_ref());
    cmd.args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    if let Some(d) = cwd {
        cmd.current_dir(d);
    }
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    Ok(cmd.spawn()?.id())
}
