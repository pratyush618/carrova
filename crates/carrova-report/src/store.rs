//! On-disk report store: one JSON file per report in a caller-chosen
//! directory, newest [`MAX_REPORTS`] kept.

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::error::ReportError;
use crate::fs::{ReportFs, write_private};
use crate::report::{ErrorReport, ReportKind};

/// Sub-directory of the front-end's state directory holding the store.
pub const REPORTS_DIR: &str = "reports";
/// Reports kept; older ones are deleted when a new one is saved.
pub const MAX_REPORTS: usize = 20;
/// Digits in a report id (zero-padded Unix nanoseconds).
const ID_LEN: usize = 20;
const EXTENSION: &str = "json";

/// Identifier of a stored report: zero-padded creation time in Unix
/// nanoseconds, so ids sort chronologically. Parsing rejects anything
/// else, so an id received over IPC can never name a path.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ReportId(String);

impl FromStr for ReportId {
    type Err = ReportError;

    fn from_str(id: &str) -> Result<Self, Self::Err> {
        if id.len() == ID_LEN && id.bytes().all(|b| b.is_ascii_digit()) {
            Ok(Self(id.to_owned()))
        } else {
            Err(ReportError::InvalidId(id.to_owned()))
        }
    }
}

impl Serialize for ReportId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// Validates like [`FromStr`]: ids arriving over IPC are untrusted.
impl<'de> Deserialize<'de> for ReportId {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

impl fmt::Display for ReportId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// One line of the report list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ReportSummary {
    /// Report id.
    pub id: ReportId,
    /// Creation time, Unix seconds.
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub when: u64,
    /// Panic or error.
    pub kind: ReportKind,
    /// First (redacted) message.
    pub headline: String,
}

/// Reports and the consent choice, stored under one directory
/// (front-ends use [`ReportStore::in_state_dir`]). Every file is written
/// private to the user through the caller's [`ReportFs`].
#[derive(Debug, Clone)]
pub struct ReportStore {
    dir: PathBuf,
    fs: Arc<dyn ReportFs>,
}

impl ReportStore {
    /// A store rooted at `dir` (created on first write), writing through
    /// `fs`.
    #[must_use]
    pub fn new(dir: impl Into<PathBuf>, fs: Arc<dyn ReportFs>) -> Self {
        Self {
            dir: dir.into(),
            fs,
        }
    }

    /// The store every front-end uses: [`REPORTS_DIR`] under Carrova's
    /// state directory (`Platform::state_dir()`).
    #[must_use]
    pub fn in_state_dir(state_dir: &Path, fs: Arc<dyn ReportFs>) -> Self {
        Self::new(state_dir.join(REPORTS_DIR), fs)
    }

    /// The store directory.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Saves `report` if the persisted consent is
    /// [`ReportConsent::Allowed`](crate::ReportConsent::Allowed) and prunes to [`MAX_REPORTS`]. Returns the
    /// new id, or `None` without consent (nothing is written). This is the
    /// single consent gate: every report goes through it.
    ///
    /// # Errors
    ///
    /// [`ReportError::Io`] if the directory or file cannot be written, or
    /// the consent file cannot be read.
    pub fn save(&self, report: &ErrorReport) -> Result<Option<ReportId>, ReportError> {
        if !self.consent()?.is_allowed() {
            return Ok(None);
        }
        let id = self.fresh_id();
        let json = serde_json::to_vec_pretty(report).map_err(ReportError::malformed(&self.dir))?;
        self.write_file(&format!("{id}.{EXTENSION}"), &json)?;
        self.prune()?;
        Ok(Some(id))
    }

    /// Write `bytes` as the private file `name` in the store directory
    /// (created if needed), atomically.
    pub(crate) fn write_file(&self, name: &str, bytes: &[u8]) -> Result<(), ReportError> {
        fs::create_dir_all(&self.dir).map_err(ReportError::io(&self.dir))?;
        let tmp = self.dir.join(format!(".{name}.tmp"));
        write_private(self.fs.as_ref(), &tmp, &self.dir.join(name), bytes)
    }

    /// Summaries of the stored reports, newest first. Unreadable or
    /// malformed files are skipped.
    ///
    /// # Errors
    ///
    /// [`ReportError::Io`] if the directory exists but cannot be listed.
    pub fn list(&self) -> Result<Vec<ReportSummary>, ReportError> {
        let summaries = self
            .ids()?
            .into_iter()
            .filter_map(|id| {
                let report = self.read(&id).ok()?;
                Some(ReportSummary {
                    when: report.when,
                    kind: report.kind,
                    headline: report.headline().to_owned(),
                    id,
                })
            })
            .collect();
        Ok(summaries)
    }

    /// The full report `id`.
    ///
    /// # Errors
    ///
    /// [`ReportError::NotFound`], [`ReportError::Io`] or [`ReportError::Malformed`].
    pub fn read(&self, id: &ReportId) -> Result<ErrorReport, ReportError> {
        let path = self.path(id);
        let bytes = fs::read(&path).map_err(|err| match err.kind() {
            std::io::ErrorKind::NotFound => ReportError::NotFound(id.to_string()),
            _ => ReportError::io(&path)(err),
        })?;
        serde_json::from_slice(&bytes).map_err(ReportError::malformed(&path))
    }

    /// Deletes report `id`.
    ///
    /// # Errors
    ///
    /// [`ReportError::NotFound`] or [`ReportError::Io`].
    pub fn delete(&self, id: &ReportId) -> Result<(), ReportError> {
        let path = self.path(id);
        fs::remove_file(&path).map_err(|err| match err.kind() {
            std::io::ErrorKind::NotFound => ReportError::NotFound(id.to_string()),
            _ => ReportError::io(&path)(err),
        })
    }

    /// Deletes every report (the consent choice stays).
    ///
    /// # Errors
    ///
    /// [`ReportError::Io`] if a report cannot be removed.
    pub fn clear(&self) -> Result<(), ReportError> {
        self.ids()?.iter().try_for_each(|id| self.delete(id))
    }

    /// Stored ids, newest first.
    fn ids(&self) -> Result<Vec<ReportId>, ReportError> {
        let entries = match fs::read_dir(&self.dir) {
            Ok(entries) => entries,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(err) => return Err(ReportError::io(&self.dir)(err)),
        };
        let mut ids: Vec<ReportId> = entries
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let path = entry.path();
                if path.extension()? != EXTENSION {
                    return None;
                }
                path.file_stem()?.to_str()?.parse().ok()
            })
            .collect();
        ids.sort_unstable_by(|a, b| b.cmp(a));
        Ok(ids)
    }

    fn prune(&self) -> Result<(), ReportError> {
        self.ids()?
            .iter()
            .skip(MAX_REPORTS)
            .try_for_each(|id| self.delete(id))
    }

    /// An id not yet used in this store (now, bumped past collisions).
    fn fresh_id(&self) -> ReportId {
        let mut nanos = crate::clock::since_epoch().as_nanos();
        loop {
            let id = ReportId(format!("{nanos:0ID_LEN$}"));
            if !self.path(&id).exists() {
                return id;
            }
            nanos += 1;
        }
    }

    fn path(&self, id: &ReportId) -> PathBuf {
        self.dir.join(format!("{id}.{EXTENSION}"))
    }
}
