//! Store file operations, all through [`Platform`] (no symlink following,
//! atomic replace, explicit syncs).

use std::io::{ErrorKind, Read, Write};
use std::path::{Path, PathBuf};

use carrova_platform::{DirEntry, FileKind, Platform, PlatformError};

use crate::{BundleError, Result};

/// Thin wrapper giving the store the few file operations it needs.
#[derive(Debug, Clone, Copy)]
pub(super) struct Fs<'p>(pub(super) &'p dyn Platform);

fn is_kind(e: &PlatformError, kind: ErrorKind) -> bool {
    matches!(e, PlatformError::Io { source, .. } if source.kind() == kind)
}

impl Fs<'_> {
    /// Whether anything (file, dir or link) exists at `path`.
    pub(super) fn exists(self, path: &Path) -> Result<bool> {
        match self.0.metadata(path) {
            Ok(_) => Ok(true),
            Err(e) if is_kind(&e, ErrorKind::NotFound) => Ok(false),
            Err(e) => Err(e.into()),
        }
    }

    /// Create every missing ancestor of `path` (inclusive). Existing
    /// ancestors are accepted as they are (the user may link `~/.local`).
    pub(super) fn create_ancestors(self, path: &Path) -> Result<()> {
        let mut missing = Vec::new();
        for dir in path.ancestors() {
            if dir.as_os_str().is_empty() || self.exists(dir)? {
                break;
            }
            missing.push(dir);
        }
        for dir in missing.into_iter().rev() {
            match self.0.create_dir(dir, None) {
                Err(e) if !is_kind(&e, ErrorKind::AlreadyExists) => return Err(e.into()),
                _ => {}
            }
        }
        Ok(())
    }

    /// Create directory `path` (parent must exist) or accept an existing
    /// real directory; a symlink or file there is an error.
    pub(super) fn ensure_dir(self, path: &Path) -> Result<()> {
        match self.0.create_dir(path, None) {
            Ok(()) => Ok(()),
            Err(e) if is_kind(&e, ErrorKind::AlreadyExists) => {
                if self.0.metadata(path)?.kind == FileKind::Dir {
                    Ok(())
                } else {
                    Err(BundleError::io(
                        path,
                        std::io::Error::other("exists and is not a directory"),
                    ))
                }
            }
            Err(e) => Err(e.into()),
        }
    }

    /// Read a regular file (never through a symlink), at most `limit`
    /// bytes.
    pub(super) fn read_capped(self, path: &Path, limit: u64) -> Result<Vec<u8>> {
        let file = self.0.open_read_nofollow(path)?;
        let mut out = Vec::new();
        file.take(limit.saturating_add(1))
            .read_to_end(&mut out)
            .map_err(|e| BundleError::io(path, e))?;
        if out.len() as u64 > limit {
            return Err(BundleError::io(
                path,
                std::io::Error::other(format!("larger than {limit} bytes")),
            ));
        }
        Ok(out)
    }

    /// Create a new file at `path` holding `bytes`, synced; replaces a
    /// stale leftover of an interrupted run.
    pub(super) fn write_new(self, path: &Path, bytes: &[u8]) -> Result<()> {
        let mut file = match self.0.create_new_file(path, None) {
            Err(e) if is_kind(&e, ErrorKind::AlreadyExists) => {
                self.0.remove_file(path)?;
                self.0.create_new_file(path, None)?
            }
            other => other?,
        };
        file.write_all(bytes)
            .and_then(|()| file.sync_all())
            .map_err(|e| BundleError::io(path, e))
    }

    /// Write `bytes` to `dest` atomically: a temp file in `tmp_dir`, then
    /// rename over `dest` and sync its directory.
    pub(super) fn write_atomic(self, tmp_dir: &Path, dest: &Path, bytes: &[u8]) -> Result<()> {
        let tmp = temp_path(tmp_dir, dest);
        self.write_new(&tmp, bytes)?;
        self.0.rename_replace(&tmp, dest)?;
        self.sync_parent(dest)
    }

    /// fsync the directory holding `path`.
    pub(super) fn sync_parent(self, path: &Path) -> Result<()> {
        match path.parent() {
            Some(dir) => Ok(self.0.sync_dir(dir)?),
            None => Ok(()),
        }
    }

    /// Entries of `dir` (sorted, links reported as links).
    pub(super) fn list(self, dir: &Path) -> Result<Vec<DirEntry>> {
        Ok(self.0.read_dir(dir)?)
    }

    /// Remove `path` recursively; it must lie inside `within`.
    pub(super) fn remove_tree(self, path: &Path, within: &Path) -> Result<()> {
        Ok(self.0.remove_dir_all_confined(path, within)?)
    }

    /// Remove one file or link.
    pub(super) fn remove_file(self, path: &Path) -> Result<()> {
        Ok(self.0.remove_file(path)?)
    }

    /// Rename `from` to `to`, which must not exist.
    pub(super) fn rename_noreplace(self, from: &Path, to: &Path) -> Result<()> {
        Ok(self.0.rename_noreplace(from, to)?)
    }
}

/// `<tmp_dir>/<file name of dest>.<pid>.part`: unique per process, so
/// two Carrova processes never write the same temp file.
pub(super) fn temp_path(tmp_dir: &Path, dest: &Path) -> PathBuf {
    let name = dest
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    tmp_dir.join(format!("{name}.{}.part", std::process::id()))
}
