//! Running a system tool for its text answer.

use std::process::{Command, Stdio};

/// Stdout of `cmd` when it ran, exited successfully and printed UTF-8;
/// `None` otherwise. No timeout: callers run only quick local tools.
pub(crate) fn stdout_of(cmd: &mut Command) -> Option<String> {
    let out = cmd.stdin(Stdio::null()).output().ok()?;
    String::from_utf8(out.stdout)
        .ok()
        .filter(|_| out.status.success())
}
