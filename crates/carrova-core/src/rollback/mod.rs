//! Rollback: replay the session journal in reverse — remove what apply
//! created, restore what it backed up (exact bytes + mode), restore handler
//! backups. Idempotent: safe to re-run after a crash mid-rollback. The
//! rollback point (journal + backup/) is kept until the user dismisses it,
//! so a rollback may run long after the import; it must never destroy data
//! the user created or changed since.
//!
//! Each undone step is marked in the journal (`Undone { seq }`), so a
//! re-run skips finished steps; every undo is itself safe to repeat if a
//! crash lands between the step and its marker. Nothing is ever followed
//! through a symlink.
//!
//! # What rollback deletes, keeps, or quarantines
//!
//! - **Removed** only when provably what apply left there: a file whose
//!   BLAKE3 + size match the bytes apply wrote, a symlink with the target
//!   apply created, an *empty* directory apply created, a handler-declared
//!   path whose fingerprint (file hash / link target / whole-tree hash)
//!   matches the one journalled when the handler finished.
//! - **Left in place** and reported in [`RollbackReport::failed`] (backup
//!   kept, retried by the next run): a file or symlink that changed since
//!   apply where apply created or replaced it (file actions, and handler
//!   files that had a backup), a non-empty directory apply created for a
//!   file action, and any backup whose original location is occupied.
//! - **Quarantined** — moved (rename, never copied or deleted) to
//!   `session_dir/rollback-trash/<seq>-<k>-<name>` and reported in
//!   [`RollbackReport::quarantined`]: trees rollback has to clear but cannot
//!   vouch for — a handler directory apply created that is not empty (tool
//!   state, keys the user added later), a handler-declared directory that
//!   changed since, a handler-declared path that did not exist before and
//!   changed since (e.g. `trustdb.gpg` rewritten by later `gpg` use), and
//!   handler output of a run that never finished. If the
//!   rename fails (e.g. the session dir is on another filesystem), the path
//!   is left in place and reported as failed. The trash is never deleted
//!   automatically, also not by [`dismiss`].

mod effects;
mod error;
mod outcome;
mod preview;
mod report;
mod undo;

use std::collections::HashSet;
use std::path::Path;

use carrova_platform::Platform;

use crate::error::{CoreError, JournalProblem, Result};
use crate::fsx;
use crate::journal::{
    self, BACKUP_DIR, JOURNAL_FILE, JOURNAL_VERSION, Journal, MIN_JOURNAL_VERSION, Record,
};
use crate::session::{Event, EventSink, RollbackOutcome};
use effects::Fs;
use outcome::Outcome;
pub use preview::{RollbackPreview, preview};
pub use report::{Quarantined, RollbackFailure, RollbackReport};
use undo::Cx;

/// Quarantine directory name inside a session directory.
pub const TRASH_DIR: &str = "rollback-trash";

/// Undo the apply recorded in `session_dir/journal.cbor`.
///
/// Returns `Ok` with [`RollbackReport::failed`] listing steps that need
/// manual attention; those are retried by the next run. A `RolledBack`
/// marker is appended once every step is undone.
pub fn rollback(
    platform: &dyn Platform,
    session_dir: &Path,
    sink: &dyn EventSink,
) -> Result<RollbackReport> {
    let (journal, records) = open_journal(platform, session_dir)?;
    let mut run = Run::new(
        Fs::real(platform),
        session_dir,
        &records,
        Some(journal),
        sink,
    );
    run.undo_all(&records)?;
    if let Some(journal) = &mut run.journal
        && run.report.failed.is_empty()
        && !matches!(records.last(), Some(Record::RolledBack))
    {
        journal.append(&Record::RolledBack)?;
    }
    Ok(run.report)
}

/// The session's journal, opened for appending, and its records; a
/// version this build cannot undo is refused.
fn open_journal(platform: &dyn Platform, session_dir: &Path) -> Result<(Journal, Vec<Record>)> {
    let (journal, records) = missing_journal(Journal::open_append(
        platform,
        &session_dir.join(JOURNAL_FILE),
    ))?;
    check_version(&records)?;
    Ok((journal, records))
}

/// The session's journal records, read only; a version this build
/// cannot undo is refused.
fn read_journal(platform: &dyn Platform, session_dir: &Path) -> Result<Vec<Record>> {
    let records = missing_journal(journal::read(platform, &session_dir.join(JOURNAL_FILE)))?;
    check_version(&records)?;
    Ok(records)
}

/// A journal that is not there is [`JournalProblem::Missing`].
fn missing_journal<T>(opened: Result<T>) -> Result<T> {
    match opened {
        Err(e) if fsx::kind_of(&e) == Some(std::io::ErrorKind::NotFound) => {
            Err(CoreError::Journal(JournalProblem::Missing))
        }
        other => other,
    }
}

/// Refuse a journal version this build cannot undo.
fn check_version(records: &[Record]) -> Result<()> {
    if let Some(Record::Begin { version, .. }) = records.first()
        && !(MIN_JOURNAL_VERSION..=JOURNAL_VERSION).contains(version)
    {
        return Err(CoreError::Journal(JournalProblem::UnsupportedVersion(
            *version,
        )));
    }
    Ok(())
}

/// One rollback run over a journal: real (marks each undone step in
/// `journal`) or a preview (`journal` is `None`; nothing changes).
struct Run<'a> {
    cx: Cx<'a>,
    journal: Option<Journal>,
    /// Records undone (earlier runs' markers + this run).
    undone: HashSet<u64>,
    report: RollbackReport,
    sink: &'a dyn EventSink,
}

impl<'a> Run<'a> {
    fn new(
        fs: Fs<'a>,
        session_dir: &Path,
        records: &'a [Record],
        journal: Option<Journal>,
        sink: &'a dyn EventSink,
    ) -> Self {
        Self {
            cx: Cx::new(fs, session_dir.join(TRASH_DIR), records),
            journal,
            undone: records
                .iter()
                .filter_map(|r| match r {
                    Record::Undone { seq } => Some(*seq),
                    _ => None,
                })
                .collect(),
            report: RollbackReport::default(),
            sink,
        }
    }

    /// Undo every step not undone yet, newest first.
    fn undo_all(&mut self, records: &[Record]) -> Result<()> {
        for (seq, rec) in records.iter().enumerate().rev() {
            if self.is_undone(seq) {
                continue;
            }
            if let Record::HandlerDir { abs: dir } = rec {
                self.undo_inside(dir, &records[..seq])?;
            }
            self.step(seq, rec)?;
        }
        Ok(())
    }

    fn is_undone(&self, seq: usize) -> bool {
        self.undone.contains(&(seq as u64))
    }

    /// File actions inside a handler dir ran before the handler: undo them
    /// first (`earlier` = records before the dir's), so only foreign
    /// content is left for quarantine.
    fn undo_inside(&mut self, dir: &Path, earlier: &[Record]) -> Result<()> {
        for (seq, rec) in earlier.iter().enumerate().rev() {
            let inside = abs_of(rec).is_some_and(|a| a != dir && a.starts_with(dir));
            if inside && !self.is_undone(seq) {
                self.step(seq, rec)?;
            }
        }
        Ok(())
    }

    /// Undo record `seq`, mark it in the journal if fully undone, report.
    fn step(&mut self, seq: usize, rec: &Record) -> Result<()> {
        let seq = seq as u64;
        self.cx.seq = seq;
        let out = self
            .cx
            .undo(rec, &self.report.failed)
            .unwrap_or_else(|why| Outcome::failed(describe(rec), why));
        self.undone.insert(seq); // handled in this run either way
        if let Some(journal) = &mut self.journal
            && out.failed.is_empty()
            && is_step(rec)
        {
            journal.append(&Record::Undone { seq })?;
        }
        self.emit(&out);
        self.report.merge(out);
        Ok(())
    }

    /// One [`Event::RollbackStep`] per path the outcome lists.
    fn emit(&self, out: &Outcome) {
        let emit = |target: &str, outcome: RollbackOutcome| {
            self.sink.emit(Event::RollbackStep {
                target: target.to_owned(),
                outcome,
            });
        };
        for t in &out.removed {
            emit(t, RollbackOutcome::Removed);
        }
        for t in &out.restored {
            emit(t, RollbackOutcome::Restored);
        }
        for (t, to) in &out.quarantined {
            tracing::warn!(target_path = %t, trash = %to.display(), "rollback quarantined path");
            emit(t, RollbackOutcome::MovedAside);
        }
        for (t, reason) in &out.failed {
            tracing::warn!(target_path = %t, %reason, "rollback step failed");
            emit(t, RollbackOutcome::Failed);
        }
    }
}

/// Delete the rollback point (journal + backups). Irreversible. The
/// quarantine (`rollback-trash/`) holds user data and is kept.
pub fn dismiss(platform: &dyn Platform, session_dir: &Path) -> Result<()> {
    for name in [BACKUP_DIR, JOURNAL_FILE] {
        if let Err(e) = platform.remove_dir_all_confined(&session_dir.join(name), session_dir) {
            let e = CoreError::from(e);
            if fsx::kind_of(&e) != Some(std::io::ErrorKind::NotFound) {
                return Err(e);
            }
        }
    }
    Ok(())
}

/// Records that change the filesystem (and so get an `Undone` marker).
fn is_step(rec: &Record) -> bool {
    !matches!(
        rec,
        Record::Begin { .. }
            | Record::Committed
            | Record::Undone { .. }
            | Record::RolledBack
            | Record::HandlerDone { .. }
    )
}

/// Filesystem path a step acts on, if it is a single path.
fn abs_of(rec: &Record) -> Option<&Path> {
    match rec {
        Record::CreatedDir { abs }
        | Record::CreatedTemp { abs }
        | Record::CreatedFile { abs, .. }
        | Record::CreatedSymlink { abs, .. }
        | Record::BackedUp { abs, .. }
        | Record::ModeChanged { abs, .. }
        | Record::HandlerDir { abs } => Some(abs),
        Record::HandlerBackup { .. }
        | Record::HandlerDone { .. }
        | Record::Begin { .. }
        | Record::Committed
        | Record::Undone { .. }
        | Record::RolledBack => None,
    }
}

/// Target shown for a step that failed as a whole.
fn describe(rec: &Record) -> String {
    match rec {
        Record::HandlerBackup { item, .. } => format!("handler backup of {item}"),
        _ => abs_of(rec)
            .map(|a| a.display().to_string())
            .unwrap_or_default(),
    }
}
