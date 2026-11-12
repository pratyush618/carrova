//! Running a system tool for its text answer.

use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

/// How long a tool may run before it is killed.
const DEADLINE: Duration = Duration::from_secs(3);
/// Most output bytes a tool may print before it is killed.
const OUTPUT_CAP: usize = 64 * 1024;
/// Pause between exit checks once the output is read.
const POLL: Duration = Duration::from_millis(10);

/// Stdout of `cmd` when it ran, exited successfully and printed text;
/// `None` otherwise. Undecodable bytes become U+FFFD: ARP.EXE writes in the
/// OEM code page, and the IP and MAC tokens callers read are ASCII.
/// A tool that runs past 3 s or prints over 64 KiB is killed with its
/// children, so a wedged tool cannot stall its caller.
pub(crate) fn stdout_of(cmd: &mut Command) -> Option<String> {
    run_bounded(cmd, DEADLINE, OUTPUT_CAP)
}

/// [`stdout_of`] with its limits as parameters.
fn run_bounded(cmd: &mut Command, limit: Duration, cap: usize) -> Option<String> {
    crate::sys::isolate_tool(cmd);
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let out = read_within(&mut child, limit, cap);
    if out.is_none() {
        // Kill the whole tree before reaping, so no child holds the pipe
        // open or lingers as a zombie.
        let _ = crate::sys::kill_tool(&mut child);
    }
    let _ = child.wait();
    out.map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
}

/// Stdout of `child` once it exited successfully, all within `limit` and
/// `cap` bytes; `None` if it did not (the caller kills it).
fn read_within(child: &mut Child, limit: Duration, cap: usize) -> Option<Vec<u8>> {
    let deadline = Instant::now() + limit;
    let stdout = child.stdout.take()?;
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut out = Vec::new();
        let read = stdout.take(cap as u64 + 1).read_to_end(&mut out);
        let _ = tx.send(read.map(|_| out));
    });
    let out = rx.recv_timeout(limit).ok()?.ok()?;
    if out.len() > cap {
        return None;
    }
    loop {
        match child.try_wait().ok()? {
            Some(status) => return status.success().then_some(out),
            None if Instant::now() < deadline => thread::sleep(POLL),
            None => return None,
        }
    }
}

#[cfg(test)]
#[path = "../tests/tool/unit.rs"]
mod tests;
