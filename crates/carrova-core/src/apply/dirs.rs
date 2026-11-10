//! Directories, symlinks and the ancestors of every target: created one by
//! one, each step journalled (and synced) before it runs.

use std::path::Path;

use carrova_platform::FileKind;

use super::{Applier, CREATED_DIR_MODE, unsafe_path};
use crate::error::{CoreError, Result};
use crate::fsx;
use crate::journal::Record;
use crate::plan::{Action, ObjectKind, PlannedAction};

impl Applier<'_> {
    /// A directory or symlink action, applied on its own (journal record
    /// synced before the step).
    pub(super) fn apply_object(&mut self, a: &PlannedAction) -> Result<()> {
        self.failpoint.check(self.done)?;
        let (abs, label) = self.target(a)?;
        let replace = matches!(a.action, Action::Replace);
        if a.kind == ObjectKind::Symlink {
            self.apply_symlink(a, &abs, &label, replace)?;
        } else {
            self.apply_dir(a, &abs, &label, replace)?;
        }
        self.finished(a, label);
        Ok(())
    }

    /// The resolved root must be a real directory: a symlinked root (even a
    /// user's own, e.g. `~/Documents`) is refused, consistent with the plan,
    /// which never writes through one. If missing it is created with its
    /// missing ancestors.
    pub(super) fn ensure_root(&mut self, root: &Path) -> Result<()> {
        match fsx::lstat(self.p, root)? {
            Some(m) if m.kind == FileKind::Dir => Ok(()),
            Some(m) if m.kind == FileKind::Symlink => Err(unsafe_path(
                root.display().to_string(),
                "root is a symlink; never written through",
            )),
            Some(_) => Err(unsafe_path(
                root.display().to_string(),
                "root is not a directory",
            )),
            None => {
                let mut missing = vec![root.to_path_buf()];
                let mut cur = root.parent();
                while let Some(c) = cur {
                    if fsx::lstat(self.p, c)?.is_some() {
                        break;
                    }
                    missing.push(c.to_path_buf());
                    cur = c.parent();
                }
                for d in missing.iter().rev() {
                    self.create_dir(d, Some(CREATED_DIR_MODE))?;
                }
                Ok(())
            }
        }
    }

    /// Walk `rel`'s parent components below `root` with lstat; create
    /// missing ones (mode 700); refuse symlinks and non-directories.
    pub(super) fn ensure_parents(&mut self, root: &Path, rel: &Path) -> Result<()> {
        let Some(parent) = rel.parent() else {
            return Ok(());
        };
        let mut cur = root.to_path_buf();
        for c in parent.components() {
            cur.push(c);
            match fsx::lstat(self.p, &cur)? {
                Some(m) if m.kind == FileKind::Dir => {}
                Some(_) => return Err(not_a_dir_ancestor(&cur)),
                None => self.create_dir(&cur, Some(CREATED_DIR_MODE))?,
            }
        }
        Ok(())
    }

    /// Create a directory (journalled first, parent synced after).
    fn create_dir(&mut self, abs: &Path, mode: Option<u32>) -> Result<()> {
        self.journal
            .append(&Record::CreatedDir { abs: abs.into() })?;
        self.p.create_dir(abs, mode)?;
        self.created_dirs.insert(abs.to_path_buf());
        if let Some(parent) = abs.parent() {
            self.p.sync_dir(parent)?;
        }
        Ok(())
    }

    fn apply_dir(
        &mut self,
        a: &PlannedAction,
        abs: &Path,
        label: &str,
        replace: bool,
    ) -> Result<()> {
        let want = a.mode.map(|m| self.policy.final_mode(m, None));
        match fsx::lstat(self.p, abs)? {
            None => self.create_dir(abs, want),
            Some(m) if m.kind == FileKind::Dir => {
                // Only ever remove bits from an existing directory.
                if let (Some(have), Some(incoming)) = (m.mode.map(|x| x & 0o777), a.mode) {
                    let fin = self.policy.final_mode(incoming, Some(have));
                    if fin != have {
                        self.journal.append(&Record::ModeChanged {
                            abs: abs.into(),
                            old_mode: have,
                        })?;
                        fsx::set_mode(self.p, abs, fin)?;
                    }
                }
                Ok(())
            }
            Some(_) if replace => {
                self.backup_existing(abs, label)?;
                self.create_dir(abs, want)
            }
            Some(_) => Err(unsafe_path(label, "target exists and is not a directory")),
        }
    }

    fn apply_symlink(
        &mut self,
        a: &PlannedAction,
        abs: &Path,
        label: &str,
        replace: bool,
    ) -> Result<()> {
        let target = a
            .link_target
            .as_ref()
            .ok_or_else(|| CoreError::Verification(format!("{label} (no symlink target)")))?;
        if replace {
            self.backup_existing(abs, label)?;
        }
        self.journal.append(&Record::CreatedSymlink {
            abs: abs.into(),
            target: target.clone(),
        })?;
        self.p.create_symlink(target, abs)?;
        let (parent, _) = fsx::split(abs)?;
        self.p.sync_dir(parent)?;
        Ok(())
    }
}

/// An ancestor of a target is a symlink or not a directory.
pub(super) fn not_a_dir_ancestor(at: &Path) -> CoreError {
    unsafe_path(
        at.display().to_string(),
        "ancestor is a symlink or not a directory",
    )
}
