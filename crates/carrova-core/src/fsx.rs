//! Small no-follow filesystem helpers shared by apply, rollback, staging,
//! the journal and the session store.

use std::io;
use std::path::{Path, PathBuf};

use carrova_platform::{FileKind, FileMeta, PermissionModel, Platform, PlatformError};

use crate::error::{CoreError, Result};

/// I/O error kind behind `e`, if any.
pub(crate) fn kind_of(e: &CoreError) -> Option<io::ErrorKind> {
    match e {
        CoreError::Platform(PlatformError::Io { source, .. }) | CoreError::Io(source) => {
            Some(source.kind())
        }
        _ => None,
    }
}

/// lstat; `None` if missing.
pub(crate) fn lstat(p: &dyn Platform, path: &Path) -> Result<Option<FileMeta>> {
    match p.metadata(path) {
        Ok(m) => Ok(Some(m)),
        Err(e) => {
            let e = CoreError::from(e);
            if kind_of(&e) == Some(io::ErrorKind::NotFound) {
                Ok(None)
            } else {
                Err(e)
            }
        }
    }
}

/// Set the permission bits of `path` where the platform has them: a no-op
/// under [`PermissionModel::Acl`], where ACLs decide access.
pub(crate) fn set_mode(p: &dyn Platform, path: &Path, mode: u32) -> Result<()> {
    if p.permission_model() == PermissionModel::Unix {
        p.set_mode(path, mode)?;
    }
    Ok(())
}

/// `(parent, file name)` of an absolute path.
pub(crate) fn split(abs: &Path) -> Result<(&Path, String)> {
    match (abs.parent(), abs.file_name()) {
        (Some(p), Some(n)) => Ok((p, n.to_string_lossy().into_owned())),
        _ => Err(CoreError::UnsafePath {
            path: abs.display().to_string(),
            reason: "no parent directory".into(),
        }),
    }
}

/// In-progress name for a backup copy.
pub(crate) fn partial_of(backup: &Path) -> PathBuf {
    let name = backup
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    backup.with_file_name(format!("{name}.partial"))
}

/// Copy the object at `from` to the new path `to` without following
/// symlinks: files (bytes, mode, mtime; fsynced), directory trees,
/// symlinks (target verbatim).
pub(crate) fn copy_tree(p: &dyn Platform, from: &Path, to: &Path) -> Result<()> {
    let m = p.metadata(from)?;
    let mode = m.mode.map(|x| x & 0o777);
    match m.kind {
        FileKind::File => {
            let mut inp = p.open_read_nofollow(from)?;
            let mut out = p.create_new_file(to, Some(mode.unwrap_or(0o600)))?;
            io::copy(&mut inp, &mut out)?;
            if let Some(t) = m.modified {
                out.set_modified(t)?;
            }
            out.sync_all()?;
        }
        FileKind::Dir => {
            p.create_dir(to, Some(0o700))?;
            for entry in p.read_dir(from)? {
                copy_tree(p, &from.join(&entry.name), &to.join(&entry.name))?;
            }
            if let Some(mode) = mode {
                set_mode(p, to, mode)?;
            }
            p.sync_dir(to)?;
        }
        FileKind::Symlink => {
            let target = p.read_link(from)?;
            let target = target.to_str().ok_or_else(|| CoreError::UnsafePath {
                path: from.display().to_string(),
                reason: "symlink target is not UTF-8".into(),
            })?;
            p.create_symlink(target, to)?;
        }
        FileKind::Other => {
            return Err(CoreError::UnsafePath {
                path: from.display().to_string(),
                reason: "not a file, directory or symlink".into(),
            });
        }
    }
    Ok(())
}

/// Remove whatever is at `path` (dir trees confined to its parent,
/// never following links). `false` if nothing was there.
pub(crate) fn remove_any(p: &dyn Platform, path: &Path) -> Result<bool> {
    let Some(m) = lstat(p, path)? else {
        return Ok(false);
    };
    let r = if m.kind == FileKind::Dir {
        let (parent, _) = split(path)?;
        p.remove_dir_all_confined(path, parent)
    } else {
        p.remove_file(path)
    };
    match r.map_err(CoreError::from) {
        Ok(()) => Ok(true),
        Err(e) if kind_of(&e) == Some(io::ErrorKind::NotFound) => Ok(false),
        Err(e) => Err(e),
    }
}

/// Move `from` to the absent path `to` (never replacing) without
/// syncing directories. Across filesystems: copy to `tmp` (sibling of
/// `to`; files fsynced), rename into place, then remove `from`.
pub(crate) fn move_noreplace(p: &dyn Platform, from: &Path, to: &Path, tmp: &Path) -> Result<()> {
    match p.rename_noreplace(from, to).map_err(CoreError::from) {
        Ok(()) => Ok(()),
        Err(e) if kind_of(&e) == Some(io::ErrorKind::CrossesDevices) => {
            remove_any(p, tmp)?;
            copy_tree(p, from, tmp)?;
            p.rename_noreplace(tmp, to)?;
            remove_any(p, from)?;
            Ok(())
        }
        Err(e) => Err(e),
    }
}

/// [`move_noreplace`], then fsync both parent directories.
pub(crate) fn move_into_place(p: &dyn Platform, from: &Path, to: &Path, tmp: &Path) -> Result<()> {
    move_noreplace(p, from, to, tmp)?;
    let (to_parent, _) = split(to)?;
    p.sync_dir(to_parent)?;
    let (from_parent, _) = split(from)?;
    p.sync_dir(from_parent)?;
    Ok(())
}

/// Create the directory `path` with `mode`; an existing directory there
/// is fine (anything else in the way is the creation error).
pub(crate) fn create_dir_or_existing(p: &dyn Platform, path: &Path, mode: u32) -> Result<()> {
    let Err(e) = p.create_dir(path, Some(mode)) else {
        return Ok(());
    };
    let e = CoreError::from(e);
    if kind_of(&e) == Some(io::ErrorKind::AlreadyExists)
        && lstat(p, path)?.is_some_and(|m| m.kind == FileKind::Dir)
    {
        Ok(())
    } else {
        Err(e)
    }
}

#[cfg(test)]
#[path = "../tests/fsx/unit.rs"]
mod tests;
