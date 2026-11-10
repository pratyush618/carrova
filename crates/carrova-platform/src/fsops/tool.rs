//! Environment for running external tools.

use std::ffi::OsStr;
use std::path::Path;

/// Minimal tool environment: `HOME`, `PATH` (if known), `LC_ALL=C`.
pub(crate) fn tool_command(bin: &Path, home: &Path, path: Option<&OsStr>) -> std::process::Command {
    let mut cmd = std::process::Command::new(bin);
    cmd.env_clear().env("HOME", home).env("LC_ALL", "C");
    if let Some(p) = path {
        cmd.env("PATH", p);
    }
    cmd
}
