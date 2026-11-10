//! What resolution reports: entries, skips and their reasons.

use std::path::PathBuf;

use carrova_platform::{FileKind, SemanticRoot};

use crate::PathRef;

/// Files of one item, as found on this machine.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResolvedItem {
    pub item_id: String,
    /// Sorted by `(root, rel)`, deduplicated. Includes directories and
    /// (non-escaping) symlinks as their own entries.
    pub entries: Vec<FileEntry>,
    /// Recipe paths that do not exist (not an error), in recipe order.
    pub missing: Vec<PathRef>,
    /// Things deliberately not included, with the reason. Sorted.
    pub skipped: Vec<Skipped>,
    /// Sum of `size` over regular-file entries.
    pub total_size: u64,
}

/// One filesystem object belonging to an item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEntry {
    pub root: SemanticRoot,
    /// Path relative to `root`, `/`-separated.
    pub rel: String,
    /// lstat kind: `File`, `Dir` or `Symlink` (never `Other`).
    pub kind: FileKind,
    /// Bytes (for symlinks: length of the target string; dirs: as reported).
    pub size: u64,
    /// Permission bits, when the OS has them.
    pub mode: Option<u32>,
    /// Raw link target, for symlinks only. Never followed.
    pub link_target: Option<PathBuf>,
}

/// Something under an item path that was left out.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Skipped {
    pub root: SemanticRoot,
    /// Path relative to `root` (lossy if not UTF-8).
    pub rel: String,
    pub reason: SkipReason,
}

/// Why an object was left out.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, thiserror::Error)]
pub enum SkipReason {
    #[error("semantic root unavailable: {0}")]
    RootUnavailable(String),
    #[error("path resolves outside its semantic root")]
    OutsideRoot,
    #[error("a parent directory `{ancestor}` is a symlink (never followed)")]
    SymlinkInPath { ancestor: String },
    #[error("symlink points outside its semantic root and home: {}", target.display())]
    SymlinkEscapesRoots { target: PathBuf },
    #[error("not a regular file, directory or symlink")]
    SpecialFile,
    #[error("file name is not valid UTF-8")]
    NonUtf8Name,
    #[error("item has an invalid exclude glob: {0}")]
    InvalidExclude(String),
    #[error("I/O error: {0}")]
    Io(String),
}
