//! Apply journal: append-only record of every filesystem step apply takes,
//! persisted (fsync) *before* the step runs, so rollback can undo a
//! partial apply after an error or a crash.
//!
//! File format (`session_dir/journal.cbor`): a sequence of frames, each
//! `u32 little-endian length` + that many bytes of one CBOR [`Record`].
//! A trailing frame that is shorter than its declared length (torn write)
//! is ignored on read and truncated before the next append. A complete
//! frame that fails to decode is corruption and an error.
//!
//! Records hold paths, BLAKE3 digests and symlink targets — never file
//! contents. The digests let rollback prove that an object is still exactly
//! what apply (or a handler run by apply) left there before removing it.
//!
//! Format version [`JOURNAL_VERSION`] is carried by the leading
//! [`Record::Begin`]; rollback accepts versions
//! [`MIN_JOURNAL_VERSION`]`..=`[`JOURNAL_VERSION`] and refuses others.
//!
//! Version history:
//! - **2** — every record fsynced on its own; a file reaches its target by
//!   renaming a sibling temp (`CreatedFile.tmp`), written only after any
//!   backup of the target was taken.
//! - **3** — records may be appended in batches sharing one fsync
//!   ([`Journal::push`] + [`Journal::sync`]); every step still runs only
//!   after its record is durable. `CreatedFile.src` (v2: `tmp`) may be the
//!   staged file itself, and the record names the `backup` taken of the
//!   target in the same batch, so rollback can tell that a rename never
//!   happened even after staging was wiped. v2 journals still roll back.

use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use carrova_manifest::{Blake3Hash, DEFAULT_CHUNK_PARAMS};
use carrova_platform::{FileKind, Platform};
use serde::{Deserialize, Serialize};

use crate::error::{CoreError, JournalProblem, Result};
use crate::fsx;
use crate::key::ItemKey;

/// Journal file name inside a session directory.
pub const JOURNAL_FILE: &str = "journal.cbor";
/// Backup directory name inside a session directory.
pub const BACKUP_DIR: &str = "backup";

/// Journal format version written in [`Record::Begin`].
pub const JOURNAL_VERSION: u8 = 3;
/// Oldest journal format version rollback still reads (an apply
/// interrupted under an older Carrova must still roll back).
pub const MIN_JOURNAL_VERSION: u8 = 2;

/// Upper bound for one frame (sanity check against a corrupt length).
const MAX_FRAME: u32 = 16 * 1024 * 1024;

/// Kind of object moved to the backup store.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackupKind {
    File,
    Dir,
    Symlink,
}

/// What an object looked like when apply finished with it. Modes and
/// timestamps are not part of it (changing them loses no data).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Fingerprint {
    /// Regular file: BLAKE3 of its bytes and its size.
    File { hash: Blake3Hash, size: u64 },
    /// Symlink: its raw target.
    Symlink { target: PathBuf },
    /// Directory: BLAKE3 over the sorted names + fingerprints of its whole
    /// tree.
    Dir { tree: Blake3Hash },
    /// Socket, FIFO or device: kind only.
    Special,
}

/// Fingerprint of the object at `path` (never following symlinks);
/// `None` if nothing is there.
pub fn fingerprint(p: &dyn Platform, path: &Path) -> Result<Option<Fingerprint>> {
    let Some(m) = fsx::lstat(p, path)? else {
        return Ok(None);
    };
    Ok(Some(match m.kind {
        FileKind::File => {
            let (hash, size) = hash_file(p, path)?;
            Fingerprint::File { hash, size }
        }
        FileKind::Symlink => Fingerprint::Symlink {
            target: p.read_link(path)?,
        },
        FileKind::Dir => {
            let mut children = Vec::new();
            for entry in p.read_dir(path)? {
                let name = entry.name;
                // Injective naming: UTF-8 names verbatim, others escaped.
                let key = match name.to_str() {
                    Some(n) => format!("u:{n}"),
                    None => format!("b:{name:?}"),
                };
                let fp = fingerprint(p, &path.join(&name))?;
                children.push((key, fp));
            }
            children.sort_by(|a, b| a.0.cmp(&b.0));
            let mut body = Vec::new();
            ciborium::into_writer(&children, &mut body)
                .map_err(|e| CoreError::Journal(JournalProblem::FingerprintEncode(e)))?;
            Fingerprint::Dir {
                tree: Blake3Hash::of(&body),
            }
        }
        FileKind::Other => Fingerprint::Special,
    }))
}

/// BLAKE3 and size of the regular file at `path` (no-follow, streaming).
pub fn hash_file(p: &dyn Platform, path: &Path) -> Result<(Blake3Hash, u64)> {
    let f = p.open_read_nofollow(path)?;
    let c = carrova_manifest::chunk_reader(std::io::BufReader::new(f), DEFAULT_CHUNK_PARAMS)?;
    Ok((c.file_hash, c.size))
}

/// One journalled step. Written *before* the step it describes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Record {
    /// Apply started (session id for diagnostics, [`JOURNAL_VERSION`]).
    Begin { session: String, version: u8 },
    /// A directory is about to be created at `abs`.
    CreatedDir { abs: PathBuf },
    /// A sibling temp file is about to be created at `abs`.
    CreatedTemp { abs: PathBuf },
    /// The complete file `src` (BLAKE3 `hash`, `size` bytes, final mode
    /// and mtime) is about to be renamed onto `abs`. `src` is the staged
    /// file or a sibling temp (v2: always the temp, field name `tmp`).
    /// While `src` exists, the rename has not happened.
    CreatedFile {
        abs: PathBuf,
        #[serde(alias = "tmp")]
        src: PathBuf,
        hash: Blake3Hash,
        size: u64,
        /// v3: where the existing target is moved first (the
        /// [`Record::BackedUp`] of the same batch); while that backup does
        /// not exist, `abs` still holds the original. `None` in v2 (the
        /// backup was complete before this record was written).
        #[serde(default)]
        backup: Option<PathBuf>,
    },
    /// A symlink with `target` is about to be created at `abs`.
    CreatedSymlink { abs: PathBuf, target: String },
    /// The existing object at `abs` is about to be moved to `backup`.
    BackedUp {
        abs: PathBuf,
        backup: PathBuf,
        kind: BackupKind,
        mode: Option<u32>,
    },
    /// An existing directory's mode is about to change from `old_mode`.
    ModeChanged { abs: PathBuf, old_mode: u32 },
    /// A directory a handler writes into did not exist before this apply
    /// (rollback removes it if empty, otherwise quarantines it).
    HandlerDir { abs: PathBuf },
    /// A handler's declared paths are about to be copied to the backup
    /// store (`None` = path did not exist before).
    HandlerBackup {
        item: ItemKey,
        files: Vec<(PathBuf, Option<PathBuf>)>,
    },
    /// The handler for `item` finished; its declared paths now look like
    /// `after` (`None` = absent). Not a step: rollback reads it to tell
    /// handler output from later changes.
    HandlerDone {
        item: ItemKey,
        after: Vec<(PathBuf, Option<Fingerprint>)>,
    },
    /// Apply finished successfully.
    Committed,
    /// Rollback undid record number `seq` (0-based frame index).
    Undone { seq: u64 },
    /// Rollback completed every step.
    RolledBack,
}

/// Open journal positioned for appending.
///
/// Records are buffered by [`Journal::push`] and written + fsynced by
/// [`Journal::sync`]; [`Journal::append`] does both. A record is an
/// *intent*: the step it describes may only run once `sync` returned.
/// Buffered records that were never synced describe no step, so losing
/// them (error, drop, crash) is harmless.
#[derive(Debug)]
pub struct Journal {
    file: File,
    path: PathBuf,
    next_seq: u64,
    /// Encoded frames not yet written.
    pending: Vec<u8>,
}

impl Journal {
    /// Create a fresh journal (mode `600`); fails if one already exists.
    pub fn create(platform: &dyn Platform, path: &Path) -> Result<Self> {
        let file = platform.create_new_file(path, Some(0o600))?;
        Ok(Self {
            file,
            path: path.to_path_buf(),
            next_seq: 0,
            pending: Vec::new(),
        })
    }

    /// Open an existing journal for appending: returns its records and
    /// truncates a torn final frame first.
    pub fn open_append(platform: &dyn Platform, path: &Path) -> Result<(Self, Vec<Record>)> {
        let (records, valid_len) = read_frames(platform, path)?;
        // No-follow + fstat: a swapped-in symlink is never written through.
        let file = platform.open_append_nofollow(path)?;
        file.set_len(valid_len)?;
        file.sync_all()?;
        let next_seq = records.len() as u64;
        Ok((
            Self {
                file,
                path: path.to_path_buf(),
                next_seq,
                pending: Vec::new(),
            },
            records,
        ))
    }

    /// Append one record and fsync (with any records pushed before it).
    /// Returns its sequence number.
    pub fn append(&mut self, record: &Record) -> Result<u64> {
        let seq = self.push(record)?;
        self.sync()?;
        Ok(seq)
    }

    /// Buffer one record without writing it; it becomes durable with the
    /// next [`Journal::sync`]. Returns its sequence number.
    pub fn push(&mut self, record: &Record) -> Result<u64> {
        let mut body = Vec::new();
        ciborium::into_writer(record, &mut body)
            .map_err(|e| CoreError::Journal(JournalProblem::Encode(e)))?;
        let len = u32::try_from(body.len())
            .ok()
            .filter(|l| *l <= MAX_FRAME)
            .ok_or(CoreError::Journal(JournalProblem::RecordTooLarge))?;
        self.pending.extend_from_slice(&len.to_le_bytes());
        self.pending.extend_from_slice(&body);
        let seq = self.next_seq;
        self.next_seq += 1;
        tracing::trace!(journal = %self.path.display(), seq, "journal record");
        Ok(seq)
    }

    /// Write every buffered record and fsync: one data sync per batch.
    pub fn sync(&mut self) -> Result<()> {
        if self.pending.is_empty() {
            return Ok(());
        }
        self.file.write_all(&self.pending)?;
        self.pending.clear();
        self.file.sync_data()?;
        Ok(())
    }
}

/// Read every complete record of the journal at `path` (read only).
pub fn read(platform: &dyn Platform, path: &Path) -> Result<Vec<Record>> {
    read_frames(platform, path).map(|(r, _)| r)
}

/// Records plus the byte length of the valid (non-torn) prefix.
fn read_frames(platform: &dyn Platform, path: &Path) -> Result<(Vec<Record>, u64)> {
    let mut bytes = Vec::new();
    platform.open_read_nofollow(path)?.read_to_end(&mut bytes)?;
    let mut records = Vec::new();
    let mut pos = 0usize;
    // Stops at the end, or at a torn length prefix / torn final record.
    while let Some(head) = bytes.get(pos..pos + 4) {
        let mut len = [0u8; 4];
        len.copy_from_slice(head);
        let len = u32::from_le_bytes(len);
        if len > MAX_FRAME {
            return Err(CoreError::Journal(JournalProblem::FrameLengthCorrupt));
        }
        let start = pos + 4;
        let Some(body) = bytes.get(start..start + len as usize) else {
            break; // torn final record
        };
        let rec: Record = ciborium::from_reader(body)
            .map_err(|e| CoreError::Journal(JournalProblem::RecordCorrupt(e)))?;
        records.push(rec);
        pos = start + len as usize;
    }
    Ok((records, pos as u64))
}

#[cfg(test)]
#[path = "../tests/journal/unit.rs"]
mod tests;
