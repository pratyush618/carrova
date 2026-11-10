//! Running a declared tool for a plugin (`carrova.run_tool`): found with
//! [`Platform::find_binary`], started with [`Platform::tool_command`]
//! (minimal environment, no shell) and **no arguments**; the plugin's
//! input is its stdin, its stdout (≤ [`MAX_IO_BYTES`]) the answer, stderr
//! is discarded. Each call may run [`TOOL_TIMEOUT`], at most what is left
//! of the run's tool time; past it the tool is killed with everything it
//! started ([`Platform::isolate_tool`], [`Platform::kill_tool`]).
//!
//! shortcut: no arguments at all — a tool that needs fixed flags needs a
//! manifest-declared argument list (shown at consent) once a plugin asks.

use std::io::{Read, Write};
use std::process::{Child, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use carrova_platform::Platform;

use crate::{MAX_IO_BYTES, RunBudget};

/// Longest one tool call may run.
pub const TOOL_TIMEOUT: Duration = Duration::from_secs(10);
/// How often a finished-reading tool is checked for exit.
const POLL: Duration = Duration::from_millis(10);

/// Run the declared tool `name` on `input`; its stdout, or why not (never
/// the tool's output). The time it ran is taken off `budget`.
pub(crate) fn run(
    platform: &dyn Platform,
    name: &str,
    input: Vec<u8>,
    budget: &mut RunBudget,
) -> Result<Vec<u8>, String> {
    let limit = TOOL_TIMEOUT.min(budget.tool_time);
    if limit.is_zero() {
        return Err(format!("`{name}`: the plugin used up its tool time"));
    }
    let bin = platform
        .find_binary(name)
        .map_err(|e| format!("`{name}`: {e}"))?
        .ok_or_else(|| format!("`{name}` is not installed"))?;
    let mut cmd = platform
        .tool_command(&bin)
        .map_err(|e| format!("`{name}`: {e}"))?;
    platform.isolate_tool(&mut cmd);
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let start = Instant::now();
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("cannot run `{name}`: {e}"))?;
    let result = finish(platform, &mut child, input, limit);
    budget.tool_time = budget.tool_time.saturating_sub(start.elapsed());
    result.map_err(|why| format!("`{name}` {why}"))
}

/// Feed `input`, collect stdout, wait for exit; kill the tool (and its
/// children, which may hold stdout open) when it runs past `limit` or its
/// output past the cap. Pipes are served by threads so neither side can
/// block the other.
fn finish(
    platform: &dyn Platform,
    child: &mut Child,
    input: Vec<u8>,
    limit: Duration,
) -> Result<Vec<u8>, String> {
    let deadline = Instant::now() + limit;
    if let Some(mut stdin) = child.stdin.take() {
        // A tool that stops reading early closes the pipe; that is its call.
        thread::spawn(move || stdin.write_all(&input));
    }
    let (tx, rx) = mpsc::channel();
    if let Some(stdout) = child.stdout.take() {
        thread::spawn(move || {
            let mut out = Vec::new();
            let read = stdout.take(MAX_IO_BYTES + 1).read_to_end(&mut out);
            let _ = tx.send(read.map(|_| out));
        });
    }
    let failed = |child: &mut Child, why: String| {
        let _ = platform.kill_tool(child);
        let _ = child.wait();
        Err(why)
    };
    let out = match rx.recv_timeout(limit) {
        Ok(Ok(out)) if out.len() as u64 > MAX_IO_BYTES => {
            return failed(
                child,
                format!("output is too large (at most {MAX_IO_BYTES} bytes)"),
            );
        }
        Ok(Ok(out)) => out,
        Ok(Err(e)) => return failed(child, format!("output could not be read: {e}")),
        Err(_) => return failed(child, "ran too long".into()),
    };
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => thread::sleep(POLL),
            Ok(None) => return failed(child, "ran too long".into()),
            Err(e) => return failed(child, format!("could not be waited for: {e}")),
        }
    };
    if !status.success() {
        return Err(format!("failed ({status})"));
    }
    Ok(out)
}
