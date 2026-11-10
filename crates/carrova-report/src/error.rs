//! Errors of the report store.

use std::io;
use std::path::PathBuf;

/// Why a report or the consent file could not be read or written.
#[derive(Debug, thiserror::Error)]
pub enum ReportError {
    /// Filesystem access failed.
    #[error("report storage at {path}: {source}")]
    Io {
        /// File or directory involved.
        path: PathBuf,
        /// Underlying error.
        #[source]
        source: io::Error,
    },
    /// A stored file is not valid report/consent JSON.
    #[error("report file {path} is malformed: {source}")]
    Malformed {
        /// File involved.
        path: PathBuf,
        /// Underlying error.
        #[source]
        source: serde_json::Error,
    },
    /// The id is not one this store produces (e.g. a path from IPC input).
    #[error("invalid report id {0:?}")]
    InvalidId(String),
    /// No report with this id exists.
    #[error("no report with id {0}")]
    NotFound(String),
}

impl ReportError {
    pub(crate) fn io(path: impl Into<PathBuf>) -> impl FnOnce(io::Error) -> Self {
        let path = path.into();
        move |source| Self::Io { path, source }
    }

    pub(crate) fn malformed(path: impl Into<PathBuf>) -> impl FnOnce(serde_json::Error) -> Self {
        let path = path.into();
        move |source| Self::Malformed { path, source }
    }
}
