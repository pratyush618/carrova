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
/// A tool that runs past 3 s or prints over 64 KiB is killed (on Unix, with
/// its children), so a wedged tool cannot stall its caller.
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
    let run = read_within(&mut child, limit, cap);
    if matches!(run, Run::Abandoned) {
        // Kill the whole tree before reaping, so no child holds the pipe
        // open and the group id cannot have been reused.
        let _ = crate::sys::kill_tool(&mut child);
    }
    let _ = child.wait();
    match run {
        Run::Done(bytes) => Some(String::from_utf8_lossy(&bytes).into_owned()),
        Run::Exited | Run::Abandoned => None,
    }
}

/// How a bounded run ended.
#[derive(Debug)]
enum Run {
    /// Exited successfully within the limits, with this stdout.
    Done(Vec<u8>),
    /// Exited unsuccessfully and is already reaped: its pid may be reused,
    /// so it must not be signalled.
    Exited,
    /// Ran past a limit or could not be read or waited for; still to kill.
    Abandoned,
}

fn read_within(child: &mut Child, limit: Duration, cap: usize) -> Run {
    let deadline = Instant::now() + limit;
    let Some(stdout) = child.stdout.take() else {
        return Run::Abandoned;
    };
    let (tx, rx) = mpsc::channel();
    let reader = thread::Builder::new()
        .name("tool-stdout".into())
        .spawn(move || {
            let mut out = Vec::new();
            let read = stdout.take(cap as u64 + 1).read_to_end(&mut out);
            let _ = tx.send(read.map(|_| out));
        });
    if reader.is_err() {
        return Run::Abandoned;
    }
    let out = match rx.recv_timeout(limit) {
        Ok(Ok(out)) if out.len() <= cap => out,
        _ => return Run::Abandoned,
    };
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return Run::Done(out),
            Ok(Some(_)) => return Run::Exited,
            Ok(None) if Instant::now() < deadline => thread::sleep(POLL),
            Ok(None) | Err(_) => return Run::Abandoned,
        }
    }
}

#[cfg(test)]
#[path = "../tests/tool/unit.rs"]
mod tests;
