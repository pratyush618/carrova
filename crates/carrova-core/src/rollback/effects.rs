//! The filesystem as one rollback run sees it, and every change the run
//! makes to it.
//!
//! A real run ([`Fs::real`]) changes the disk. A preview ([`Fs::preview`])
//! changes nothing: it records each change in an overlay and answers
//! later questions ("is anything at this path?", "what is its
//! fingerprint?", "is this directory empty?") as if the changes had been
//! made, so the very same undo logic predicts what a real run would do.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use carrova_platform::{FileKind, Platform};

use super::error::{RollbackStepError as E, StepResult};
use crate::error::CoreError;
use crate::fsx;
use crate::journal::{self, Fingerprint};

/// Changes a preview would have made so far.
#[derive(Debug, Default)]
struct Overlay {
    /// Paths removed or moved away (their subtrees with them).
    gone: HashSet<PathBuf>,
    /// Path → the file that would have been moved there (a backup).
    placed: HashMap<PathBuf, PathBuf>,
}

impl Overlay {
    fn is_gone(&self, path: &Path) -> bool {
        path.ancestors().any(|a| self.gone.contains(a))
    }

    fn take_away(&mut self, path: &Path) {
        self.placed.remove(path);
        self.gone.insert(path.to_path_buf());
    }
}

/// See the [module docs](self).
pub(super) struct Fs<'a> {
    p: &'a dyn Platform,
    /// `Some` for a preview.
    overlay: Option<RefCell<Overlay>>,
}

impl<'a> Fs<'a> {
    /// The real filesystem of `p`.
    pub(super) fn real(p: &'a dyn Platform) -> Self {
        Self { p, overlay: None }
    }

    /// `p`, read-only, seen through the changes of the run so far.
    pub(super) fn preview(p: &'a dyn Platform) -> Self {
        Self {
            p,
            overlay: Some(RefCell::default()),
        }
    }

    /// The path to inspect for `path`: `None` if a preview removed it,
    /// the backup a preview restored there, else `path` itself.
    fn actual(&self, path: &Path) -> Option<PathBuf> {
        let Some(overlay) = &self.overlay else {
            return Some(path.to_path_buf());
        };
        let overlay = overlay.borrow();
        if let Some(from) = overlay.placed.get(path) {
            return Some(from.clone());
        }
        (!overlay.is_gone(path)).then(|| path.to_path_buf())
    }

    /// Kind of what is at `path` (never following links), if anything.
    pub(super) fn kind_at(&self, path: &Path) -> StepResult<Option<FileKind>> {
        match self.actual(path) {
            Some(actual) => Ok(fsx::lstat(self.p, &actual)?.map(|m| m.kind)),
            None => Ok(None),
        }
    }

    /// Fingerprint of what is at `path`, if anything.
    pub(super) fn fingerprint(&self, path: &Path) -> StepResult<Option<Fingerprint>> {
        match self.actual(path) {
            Some(actual) => Ok(journal::fingerprint(self.p, &actual)?),
            None => Ok(None),
        }
    }

    /// Target of the symlink at `path`.
    pub(super) fn read_link(&self, path: &Path) -> StepResult<PathBuf> {
        Ok(self.p.read_link(path)?)
    }

    /// Whether the directory at `path` has no entries.
    pub(super) fn is_empty_dir(&self, path: &Path) -> StepResult<bool> {
        let entries = self.p.read_dir(path)?;
        let Some(overlay) = &self.overlay else {
            return Ok(entries.is_empty());
        };
        let overlay = overlay.borrow();
        let left = entries
            .iter()
            .any(|e| !overlay.is_gone(&path.join(&e.name)));
        let placed = overlay.placed.keys().any(|p| p.parent() == Some(path));
        Ok(!left && !placed)
    }

    /// Whether `a` and `b` are regular files with the same fingerprint.
    pub(super) fn same_file(&self, a: &Path, b: &Path) -> StepResult<bool> {
        match (self.fingerprint(a)?, self.fingerprint(b)?) {
            (Some(fa @ Fingerprint::File { .. }), Some(fb)) => Ok(fa == fb),
            _ => Ok(false),
        }
    }

    /// Remove whatever is at `path`.
    pub(super) fn remove(&self, path: &Path) -> StepResult<()> {
        match &self.overlay {
            Some(overlay) => overlay.borrow_mut().take_away(path),
            None => {
                fsx::remove_any(self.p, path)?;
            }
        }
        Ok(())
    }

    /// Move `from` to the absent `to` (via `tmp` across filesystems).
    pub(super) fn move_into_place(&self, from: &Path, to: &Path, tmp: &Path) -> StepResult<()> {
        match &self.overlay {
            Some(overlay) => {
                let mut overlay = overlay.borrow_mut();
                overlay.take_away(from);
                overlay.gone.remove(to);
                overlay.placed.insert(to.to_path_buf(), from.to_path_buf());
            }
            None => fsx::move_into_place(self.p, from, to, tmp)?,
        }
        Ok(())
    }

    /// Set the permission bits of `path`.
    pub(super) fn set_mode(&self, path: &Path, mode: u32) -> StepResult<()> {
        if self.overlay.is_none() {
            self.p.set_mode(path, mode)?;
        }
        Ok(())
    }

    /// Move `abs` into `trash` under the first free name `name(k)`, never
    /// replacing anything there; returns where it went. Fails (leaving
    /// `abs` in place) across filesystems.
    pub(super) fn move_aside(
        &self,
        abs: &Path,
        trash: &Path,
        name: impl Fn(u32) -> String,
    ) -> StepResult<PathBuf> {
        if let Some(overlay) = &self.overlay {
            overlay.borrow_mut().take_away(abs);
            return Ok(trash.join(name(0)));
        }
        self.ensure_dir(trash)?;
        let (parent, _) = fsx::split(abs)?;
        for k in 0..1000u32 {
            let dest = trash.join(name(k));
            match self.p.rename_noreplace(abs, &dest).map_err(CoreError::from) {
                Ok(()) => {
                    self.p.sync_dir(trash)?;
                    self.p.sync_dir(parent)?;
                    return Ok(dest);
                }
                Err(e) if fsx::kind_of(&e) == Some(std::io::ErrorKind::AlreadyExists) => {}
                Err(e) if fsx::kind_of(&e) == Some(std::io::ErrorKind::CrossesDevices) => {
                    return Err(E::TrashOnOtherFilesystem);
                }
                Err(e) => return Err(e.into()),
            }
        }
        Err(E::NoTrashName)
    }

    /// The directory `dir` exists (created `700` if missing).
    fn ensure_dir(&self, dir: &Path) -> StepResult<()> {
        match self.kind_at(dir)? {
            None => match self.p.create_dir(dir, Some(0o700)).map_err(CoreError::from) {
                Err(e) if fsx::kind_of(&e) != Some(std::io::ErrorKind::AlreadyExists) => {
                    Err(e.into())
                }
                _ => Ok(()),
            },
            Some(FileKind::Dir) => Ok(()),
            Some(_) => Err(E::TrashNotADir),
        }
    }
}
