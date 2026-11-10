//! The session's rollback store (`backup/<n>`): existing targets are moved
//! there (journalled first) before anything replaces them.

use std::path::{Path, PathBuf};

use carrova_platform::FileKind;

use super::{Applier, unsafe_path};
use crate::error::Result;
use crate::fsx;
use crate::journal::{BackupKind, Record};

impl Applier<'_> {
    /// Buffer the intent to move whatever exists at `abs` to the backup
    /// store. Returns the backup path, `None` if nothing is there.
    pub(super) fn backup_intent(&mut self, abs: &Path, label: &str) -> Result<Option<PathBuf>> {
        let Some(m) = fsx::lstat(self.p, abs)? else {
            return Ok(None);
        };
        let kind = match m.kind {
            FileKind::File => BackupKind::File,
            FileKind::Dir => BackupKind::Dir,
            FileKind::Symlink => BackupKind::Symlink,
            FileKind::Other => return Err(unsafe_path(label, "existing target is not a file")),
        };
        let backup = self.next_backup();
        self.journal.push(&Record::BackedUp {
            abs: abs.into(),
            backup: backup.clone(),
            kind,
            mode: m.mode,
        })?;
        Ok(Some(backup))
    }

    /// Move whatever exists at `abs` to the backup store now (journalled).
    pub(super) fn backup_existing(&mut self, abs: &Path, label: &str) -> Result<()> {
        let Some(backup) = self.backup_intent(abs, label)? else {
            return Ok(());
        };
        self.journal.sync()?;
        fsx::move_into_place(self.p, abs, &backup, &fsx::partial_of(&backup))?;
        self.report.backed_up.push(label.to_string());
        Ok(())
    }

    /// A fresh, never used backup path.
    pub(super) fn next_backup(&mut self) -> PathBuf {
        self.backup_seq += 1;
        self.backup_dir.join(self.backup_seq.to_string())
    }
}
