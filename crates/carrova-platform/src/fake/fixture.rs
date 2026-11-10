//! Filesystem fixture helpers for tests of service crates, which must never
//! need `std::os::*` themselves.

use std::path::Path;

/// Create a symlink at `link` pointing to `target` (test fixtures only).
pub fn symlink(target: impl AsRef<Path>, link: impl AsRef<Path>) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link)
    }
    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_file(target, link)
    }
}

/// Set this process's umask to `mask` (test fixtures only): fixtures whose
/// modes are compared against goldens must not depend on the host's umask
/// (directories created implicitly, and symlink modes on macOS, follow it).
/// Process-wide: call it before creating any fixture, from a test binary that
/// wants the same mask everywhere.
#[cfg(unix)]
#[allow(
    clippy::useless_conversion,
    reason = "mode_t is u16 on macOS but u32 on Linux"
)]
pub fn pin_umask(mask: u16) {
    rustix::process::umask(rustix::fs::Mode::from_raw_mode(mask.into()));
}

/// Write an executable `#!/bin/sh` script `dir/name` with body `sh_body`
/// (a fake external tool for handler tests); fails if it exists. Unix only.
///
/// A short-lived `sh` writes it, so this process never holds a write fd on
/// the script: a fork elsewhere in the test binary would inherit that fd
/// until its exec, and running the script meanwhile fails with `ETXTBSY`.
#[cfg(unix)]
pub fn write_tool_script(
    dir: impl AsRef<Path>,
    name: &str,
    sh_body: &str,
) -> std::io::Result<std::path::PathBuf> {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let path = dir.as_ref().join(name);
    // `set -C`: refuse an existing file; chmod gives the exact mode.
    let mut sh = Command::new("/bin/sh")
        .args(["-c", r#"set -C && cat > "$1" && chmod 755 "$1""#, "sh"])
        .arg(&path)
        .stdin(Stdio::piped())
        .spawn()?;
    let mut stdin = sh
        .stdin
        .take()
        .ok_or_else(|| std::io::Error::other("no stdin"))?;
    stdin.write_all(b"#!/bin/sh\n")?;
    stdin.write_all(sh_body.as_bytes())?;
    drop(stdin);
    let status = sh.wait()?;
    if !status.success() {
        return Err(std::io::Error::other(format!(
            "could not write tool script {}: sh {status}",
            path.display()
        )));
    }
    Ok(path)
}
