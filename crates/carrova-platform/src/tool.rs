//! Running a system tool for its text answer.

use std::process::{Command, Stdio};

/// Stdout of `cmd` when it ran, exited successfully and printed text;
/// `None` otherwise. Undecodable bytes become U+FFFD: ARP.EXE writes in the
/// OEM code page, and the IP and MAC tokens callers read are ASCII.
/// No timeout: callers run only quick local tools.
pub(crate) fn stdout_of(cmd: &mut Command) -> Option<String> {
    let out = cmd.stdin(Stdio::null()).output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

#[cfg(test)]
#[path = "../tests/tool/unit.rs"]
mod tests;
