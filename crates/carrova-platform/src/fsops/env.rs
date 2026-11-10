//! Per-user environment shared by the Unix adapters: `HOME`, root
//! confinement, `PATH` lookup and the environment of external tools.

use std::ffi::OsStr;
use std::os::unix::fs::PermissionsExt;
use std::path::{Component, Path, PathBuf};

use crate::{PlatformError, SemanticRoot};

/// `$HOME` as the [`SemanticRoot::Home`] root: set, non-empty, absolute and
/// not the filesystem root.
pub(crate) fn home_dir(home: Option<&OsStr>) -> Result<PathBuf, PlatformError> {
    let unavailable = |reason: &str| PlatformError::RootUnavailable {
        root: SemanticRoot::Home,
        reason: reason.into(),
    };
    let home = Path::new(
        home.filter(|h| !h.is_empty())
            .ok_or_else(|| unavailable("HOME is not set"))?,
    );
    if home.is_absolute() {
        confine(SemanticRoot::Home, home)
    } else {
        Err(unavailable("HOME is not an absolute path"))
    }
}

/// Lexically normalise an absolute root candidate (no filesystem access) and
/// refuse anything that is, or climbs above, the filesystem root.
///
/// A root of `/` would make "inside a semantic root" mean "anywhere", which
/// disables every confinement check built on top of roots.
pub(crate) fn confine(root: SemanticRoot, path: &Path) -> Result<PathBuf, PlatformError> {
    let unavailable = |reason: &str| PlatformError::RootUnavailable {
        root,
        reason: reason.into(),
    };
    let mut out = PathBuf::new();
    let mut depth = 0usize;
    for c in path.components() {
        match c {
            Component::RootDir => out.push(c),
            Component::CurDir => {}
            Component::ParentDir => {
                if depth == 0 {
                    return Err(unavailable("path climbs above the filesystem root"));
                }
                out.pop();
                depth -= 1;
            }
            Component::Normal(n) => {
                out.push(n);
                depth += 1;
            }
            Component::Prefix(_) => return Err(unavailable("unexpected path prefix")),
        }
    }
    if !out.has_root() {
        return Err(unavailable("path is not absolute"));
    }
    if depth == 0 {
        return Err(unavailable("path is the filesystem root"));
    }
    Ok(out)
}

/// First executable regular file `name` in the absolute directories of
/// `path_var` (a `PATH` value). Only bare names are looked up.
pub(crate) fn find_binary(path_var: Option<&OsStr>, name: &str) -> Option<PathBuf> {
    // Bare names only: no path separators, no `.`/`..` tricks.
    if name.is_empty() || name.contains('/') || name == "." || name == ".." {
        return None;
    }
    std::env::split_paths(path_var?)
        .filter(|dir| dir.is_absolute())
        .map(|dir| dir.join(name))
        .find(|candidate| is_executable_file(candidate))
}

fn is_executable_file(path: &Path) -> bool {
    // Follows symlinks on purpose: installed binaries are often symlinks.
    std::fs::metadata(path)
        .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

/// Minimal tool environment: `HOME`, `PATH` (if known), `LC_ALL=C`.
pub(crate) fn tool_command(bin: &Path, home: &Path, path: Option<&OsStr>) -> std::process::Command {
    let mut cmd = std::process::Command::new(bin);
    cmd.env_clear().env("HOME", home).env("LC_ALL", "C");
    if let Some(p) = path {
        cmd.env("PATH", p);
    }
    cmd
}

/// Start the tool in a new process group led by itself.
pub(crate) fn isolate_tool(cmd: &mut std::process::Command) {
    use std::os::unix::process::CommandExt;
    cmd.process_group(0);
}

/// `SIGKILL` the tool's process group (its id is the tool's pid), then the
/// tool itself in case it left the group.
pub(crate) fn kill_tool(child: &mut std::process::Child) -> std::io::Result<()> {
    use rustix::process::{Pid, Signal, kill_process_group};
    match kill_process_group(Pid::from_child(child), Signal::KILL) {
        // The whole group is already gone.
        Ok(()) | Err(rustix::io::Errno::SRCH) => {}
        Err(e) => return Err(e.into()),
    }
    child.kill()
}
