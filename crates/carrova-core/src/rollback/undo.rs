//! Undoing one journal record (see the module docs for what is removed,
//! kept or quarantined). Every undo is safe to repeat.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use carrova_manifest::Blake3Hash;
use carrova_platform::FileKind;

use super::RollbackFailure;
use super::effects::Fs;
use super::error::{RollbackStepError as E, StepResult};
use super::outcome::Outcome;
use crate::fsx;
use crate::journal::{Fingerprint, Record};
use crate::key::ItemKey;

/// `HandlerDone.after` of one handler: its declared paths' fingerprints.
type HandlerAfter<'a> = &'a [(PathBuf, Option<Fingerprint>)];

/// Context shared by every undo of one rollback run.
pub(super) struct Cx<'a> {
    fs: Fs<'a>,
    trash: PathBuf,
    /// `HandlerDone.after` per handler item.
    after: HashMap<&'a ItemKey, HandlerAfter<'a>>,
    /// Sequence number of the record being undone (quarantine naming).
    pub(super) seq: u64,
}

impl<'a> Cx<'a> {
    /// Undo `records` on `fs` (real or preview), quarantining into
    /// `trash`.
    pub(super) fn new(fs: Fs<'a>, trash: PathBuf, records: &'a [Record]) -> Self {
        let after = records
            .iter()
            .filter_map(|r| match r {
                Record::HandlerDone { item, after } => Some((item, after.as_slice())),
                _ => None,
            })
            .collect();
        Self {
            fs,
            trash,
            after,
            seq: 0,
        }
    }

    /// Undo one record. `failed` = failures so far in this run (targets).
    pub(super) fn undo(&self, rec: &Record, failed: &[RollbackFailure]) -> StepResult<Outcome> {
        match rec {
            Record::Begin { .. }
            | Record::Committed
            | Record::Undone { .. }
            | Record::RolledBack
            | Record::HandlerDone { .. } => Ok(Outcome::default()),
            Record::CreatedTemp { abs } => self.undo_temp(abs),
            Record::CreatedFile {
                abs,
                src,
                hash,
                size,
                backup,
            } => self.undo_file(abs, src, (*hash, *size), backup.as_deref()),
            Record::CreatedSymlink { abs, target } => self.undo_symlink(abs, target),
            Record::CreatedDir { abs } => self.undo_dir(abs),
            Record::HandlerDir { abs } => self.undo_handler_dir(abs, failed),
            Record::ModeChanged { abs, old_mode } => self.undo_mode(abs, *old_mode),
            Record::BackedUp { abs, backup, .. } => self.restore_backup(abs, backup),
            Record::HandlerBackup { item, files } => Ok(self.undo_handler_backup(item, files)),
        }
    }

    fn undo_temp(&self, abs: &Path) -> StepResult<Outcome> {
        match self.fs.kind_at(abs)? {
            None => Ok(Outcome::default()),
            Some(FileKind::File) => {
                self.fs.remove(abs)?;
                Ok(Outcome::default())
            }
            Some(_) => Err(E::TempNotAFile),
        }
    }

    /// A file renamed (or about to be renamed) from `src` onto `abs`.
    fn undo_file(
        &self,
        abs: &Path,
        src: &Path,
        (hash, size): (Blake3Hash, u64),
        backup: Option<&Path>,
    ) -> StepResult<Outcome> {
        // Source still there → the rename never happened; `abs` (if any) is
        // not ours. (A temp source is removed by its own record; a staged
        // one by the staging wipe.)
        if self.fs.kind_at(src)?.is_some() {
            return Ok(Outcome::default());
        }
        // The target's backup was never taken → the original is still at
        // `abs` and the rename never happened either (staging may be wiped
        // already, so the source check alone cannot tell). Only undoing
        // this record's own `BackedUp` restores that backup, and that runs
        // after this one.
        if let Some(b) = backup
            && self.fs.kind_at(b)?.is_none()
        {
            return Ok(Outcome::default());
        }
        match self.fs.fingerprint(abs)? {
            None => Ok(Outcome::default()),
            Some(Fingerprint::File { hash: h, size: n }) if h == hash && n == size => {
                self.fs.remove(abs)?;
                Ok(Outcome::removed(abs))
            }
            Some(Fingerprint::File { .. }) => Err(E::Changed),
            Some(_) => Err(E::NotAFile),
        }
    }

    fn undo_symlink(&self, abs: &Path, target: &str) -> StepResult<Outcome> {
        match self.fs.kind_at(abs)? {
            None => Ok(Outcome::default()),
            Some(FileKind::Symlink) => {
                if self.fs.read_link(abs)?.as_os_str() != std::ffi::OsStr::new(target) {
                    return Err(E::LinkTargetChanged);
                }
                self.fs.remove(abs)?;
                Ok(Outcome::removed(abs))
            }
            Some(_) => Err(E::NotASymlink),
        }
    }

    fn undo_dir(&self, abs: &Path) -> StepResult<Outcome> {
        match self.fs.kind_at(abs)? {
            None => Ok(Outcome::default()),
            Some(FileKind::Dir) => {
                if !self.fs.is_empty_dir(abs)? {
                    return Err(E::DirNotEmpty);
                }
                self.fs.remove(abs)?;
                Ok(Outcome::removed(abs))
            }
            Some(_) => Err(E::NotADir),
        }
    }

    /// A handler directory apply created: removed if empty, else
    /// quarantined — unless entries rollback left in place are inside.
    fn undo_handler_dir(&self, abs: &Path, failed: &[RollbackFailure]) -> StepResult<Outcome> {
        let blocked = failed.iter().any(|f| {
            let t = Path::new(&f.target);
            t != abs && t.starts_with(abs)
        });
        if blocked {
            return Err(E::HoldsLeftovers);
        }
        match self.fs.kind_at(abs)? {
            None => Ok(Outcome::default()),
            Some(FileKind::Dir) if self.fs.is_empty_dir(abs)? => {
                self.fs.remove(abs)?;
                Ok(Outcome::removed(abs))
            }
            Some(FileKind::Dir) => self.quarantine(abs),
            Some(_) => Err(E::NotADir),
        }
    }

    fn undo_mode(&self, abs: &Path, old_mode: u32) -> StepResult<Outcome> {
        match self.fs.kind_at(abs)? {
            Some(FileKind::Dir) => {
                self.fs.set_mode(abs, old_mode)?;
                Ok(Outcome::restored(abs))
            }
            _ => Ok(Outcome::default()),
        }
    }

    /// Every declared path of a handler, last first; failures are per path.
    fn undo_handler_backup(&self, item: &ItemKey, files: &[(PathBuf, Option<PathBuf>)]) -> Outcome {
        let after = self.after.get(item).copied();
        let mut out = Outcome::default();
        for (abs, backup) in files.iter().rev() {
            match self.undo_handler_path(abs, backup.as_deref(), after) {
                Ok(o) => out.merge(o),
                Err(why) => out.failed.push((abs.display().to_string(), why)),
            }
        }
        out
    }

    /// Undo one handler-declared path: clear what is there now (remove if
    /// it is exactly the handler's output, else leave or quarantine), then
    /// put the backup back. `after` = `None` when the handler never finished.
    fn undo_handler_path(
        &self,
        abs: &Path,
        backup: Option<&Path>,
        after: Option<HandlerAfter<'_>>,
    ) -> StepResult<Outcome> {
        if let Some(b) = backup {
            // Leftover from a backup copy that never completed.
            self.fs.remove(&fsx::partial_of(b))?;
            if self.fs.kind_at(b)?.is_none() {
                return Ok(Outcome::default()); // already restored (or never taken)
            }
        }
        let mut out = Outcome::default();
        if let Some(current) = self.fs.fingerprint(abs)? {
            if let Some(b) = backup
                && self.fs.fingerprint(b)?.as_ref() == Some(&current)
            {
                // Unchanged from the original: the backup copy is redundant.
                self.fs.remove(b)?;
                return Ok(Outcome::restored(abs));
            }
            let expected = after.map(|a| {
                a.iter()
                    .find(|(x, _)| x == abs)
                    .and_then(|(_, fp)| fp.as_ref())
            });
            match expected {
                Some(Some(fp)) if *fp == current => {
                    self.fs.remove(abs)?;
                    if backup.is_none() {
                        out.removed.push(abs.display().to_string());
                    }
                }
                // Handler finished, then a file it replaced changed:
                // restoring the backup would overwrite the newer version.
                Some(_) if backup.is_some() && !matches!(current, Fingerprint::Dir { .. }) => {
                    return Err(E::Changed);
                }
                // A changed tree, a changed path that did not exist before
                // the apply, or output of a handler that never finished.
                _ => out.merge(self.quarantine(abs)?),
            }
        }
        if let Some(b) = backup {
            out.merge(self.restore_backup(abs, b)?);
        }
        Ok(out)
    }

    /// Move `abs` into the session's trash (`<seq>-<k>-<name>`), never
    /// replacing anything there. Fails (leaving `abs` in place) across
    /// filesystems.
    fn quarantine(&self, abs: &Path) -> StepResult<Outcome> {
        let (_, name) = fsx::split(abs)?;
        let seq = self.seq;
        let dest = self
            .fs
            .move_aside(abs, &self.trash, |k| format!("{seq}-{k}-{name}"))?;
        Ok(Outcome {
            quarantined: vec![(abs.display().to_string(), dest)],
            ..Outcome::default()
        })
    }

    /// Move `backup` back to the absent `abs`. Missing backup = already
    /// restored (or the move never happened). An existing `abs` is never
    /// overwritten: it is only accepted if byte-identical to the backup
    /// (crash during a cross-device backup copy), in which case the
    /// redundant copy is dropped.
    fn restore_backup(&self, abs: &Path, backup: &Path) -> StepResult<Outcome> {
        if self.fs.kind_at(backup)?.is_none() {
            return Ok(Outcome::default());
        }
        if self.fs.kind_at(abs)?.is_some() {
            if self.fs.same_file(abs, backup)? {
                self.fs.remove(backup)?;
                return Ok(Outcome::restored(abs));
            }
            return Err(E::Occupied);
        }
        let (parent, name) = fsx::split(abs)?;
        let tmp = parent.join(format!(".{name}.carrova-restore"));
        self.fs.move_into_place(backup, abs, &tmp)?;
        Ok(Outcome::restored(abs))
    }
}
