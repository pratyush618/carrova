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

/// Write an executable `#!/bin/sh` script `dir/name` with body `sh_body`
/// (a fake external tool for handler tests). The file is fsynced and closed
/// before returning; unix only.
///
/// Exec of a just-written script can fail with `ETXTBSY` if another thread
/// forks while the write fd is open: write tool scripts from one test
/// thread (e.g. a single test function per test binary).
#[cfg(unix)]
pub fn write_tool_script(
    dir: impl AsRef<Path>,
    name: &str,
    sh_body: &str,
) -> std::io::Result<std::path::PathBuf> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let path = dir.as_ref().join(name);
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o755)
        .open(&path)?;
    f.write_all(b"#!/bin/sh\n")?;
    f.write_all(sh_body.as_bytes())?;
    f.sync_all()?;
    drop(f);
    // Exact mode regardless of umask.
    std::fs::set_permissions(&path, std::os::unix::fs::PermissionsExt::from_mode(0o755))?;
    Ok(path)
}
