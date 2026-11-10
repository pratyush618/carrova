//! File primitives of the session store: private directories, atomic
//! replacement and bounded no-follow reads.

use std::io::{Read, Write};
use std::path::Path;

use carrova_platform::{FileKind, Platform};

use crate::error::{CoreError, Result};
use crate::fsx;

/// Largest document the store reads (corrupt/hostile file guard).
const MAX_DOC_LEN: u64 = 64 * 1024 * 1024;

/// Create `path` (and missing parents) as directories with `mode`; an
/// existing directory is accepted, anything else (file, symlink) is refused.
pub(crate) fn ensure_dir(platform: &dyn Platform, path: &Path, mode: u32) -> Result<()> {
    match fsx::lstat(platform, path)? {
        Some(m) if m.kind == FileKind::Dir => Ok(()),
        Some(_) => Err(CoreError::NotADirectory(path.to_path_buf())),
        None => {
            if let Some(parent) = path.parent()
                && parent != path
                && !parent.as_os_str().is_empty()
            {
                ensure_dir(platform, parent, mode)?;
            }
            fsx::create_dir_or_existing(platform, path, mode)
        }
    }
}

/// Atomically replace `dir/name` with `bytes` (temp + fsync + rename + dir fsync).
pub(crate) fn write_atomic(
    platform: &dyn Platform,
    dir: &Path,
    name: &str,
    bytes: &[u8],
) -> Result<()> {
    let tmp = dir.join(format!(".{name}.tmp"));
    // A stale temp from a crash: remove it (remove_file never follows).
    if fsx::lstat(platform, &tmp)?.is_some() {
        platform.remove_file(&tmp)?;
    }
    let result = (|| -> Result<()> {
        let mut f = platform.create_new_file(&tmp, Some(0o600))?;
        f.write_all(bytes)?;
        f.sync_all()?;
        drop(f);
        platform.rename_replace(&tmp, &dir.join(name))?;
        platform.sync_dir(dir)?;
        Ok(())
    })();
    if result.is_err() && fsx::lstat(platform, &tmp).ok().flatten().is_some() {
        let _ = platform.remove_file(&tmp);
    }
    result
}

/// Read a whole regular file without following a symlink at `path`.
/// `None` if it does not exist.
pub(crate) fn read_file(platform: &dyn Platform, path: &Path) -> Result<Option<Vec<u8>>> {
    let Some(file) = open_existing(platform, path)? else {
        return Ok(None);
    };
    let mut buf = Vec::new();
    file.take(MAX_DOC_LEN + 1).read_to_end(&mut buf)?;
    if buf.len() as u64 > MAX_DOC_LEN {
        return Err(CoreError::FileTooLarge(path.to_path_buf()));
    }
    Ok(Some(buf))
}

/// Open `path` for reading without following a symlink; `None` if it does
/// not exist.
pub(super) fn open_existing(platform: &dyn Platform, path: &Path) -> Result<Option<std::fs::File>> {
    match platform.open_read_nofollow(path) {
        Ok(f) => Ok(Some(f)),
        Err(e) => {
            let e = CoreError::from(e);
            if fsx::kind_of(&e) == Some(std::io::ErrorKind::NotFound) {
                Ok(None)
            } else {
                Err(e)
            }
        }
    }
}
