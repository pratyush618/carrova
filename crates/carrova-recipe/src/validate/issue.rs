//! Validation problems and how they print.

use std::fmt;
use std::path::PathBuf;

use carrova_platform::OsKind;

use crate::PathRef;

/// One validation problem. [`fmt::Display`] gives a single human line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    /// Recipe file, when known.
    pub file: Option<PathBuf>,
    /// Recipe id (may itself be invalid).
    pub recipe: String,
    /// Field location, e.g. `items[2].exclude[0]`; empty = whole recipe.
    pub location: String,
    pub kind: IssueKind,
}

/// What is wrong.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum IssueKind {
    #[error("unsupported schema version {found}")]
    UnsupportedSchema { found: u32 },
    #[error("`{value}` is not lowercase kebab-case")]
    NotKebab { value: String },
    #[error("must not be empty")]
    Empty,
    #[error("no detection rules for linux")]
    NoLinuxDetect,
    #[error("no detection rules for any OS")]
    NoDetect,
    #[error("binary name `{name}` must be a bare name (no path separators, not empty)")]
    InvalidBinaryName { name: String },
    #[error("process name `{name}` must be a bare name (no path separators, not empty)")]
    InvalidProcessName { name: String },
    #[error("`{field}` needs recipe schema {min} or later")]
    RequiresSchema { field: &'static str, min: u32 },
    #[error("unmovable item must not name a handler")]
    UnmovableWithHandler,
    #[error("recipe has no items")]
    NoItems,
    #[error("duplicate item id `{id}`")]
    DuplicateItemId { id: String },
    #[error("item has no paths (only unmovable items may omit them)")]
    NoPaths,
    #[error(
        "{} override has {found} path(s) but the item has {base}; per-OS paths must match `paths` slot by slot",
        os.display_name()
    )]
    OsPathsMisaligned {
        os: OsKind,
        base: usize,
        found: usize,
    },
    #[error("invalid exclude glob `{glob}`: {message}")]
    InvalidGlob { glob: String, message: String },
    #[error("sensitive item must not be on by default (`default: true`)")]
    SensitiveDefaultOn,
    #[error("unmovable item must not be on by default (`default: true`)")]
    UnmovableDefaultOn,
    #[error("category `{category}` does not match directory `{dir}`")]
    CategoryDirMismatch { category: String, dir: String },
    #[error("id `{id}` does not match directory `{dir}`")]
    AppDirMismatch { id: String, dir: String },
    #[error("recipe id `{id}` also used by {other}")]
    DuplicateRecipeId { id: String, other: PathBuf },
    #[error(
        "sensitive path `{path}` (item `{sensitive_item}`) would be captured by non-sensitive item `{by_item}`; exclude it there"
    )]
    SensitiveCapturedByNonSensitive {
        path: PathRef,
        sensitive_item: String,
        by_item: String,
    },
}

impl fmt::Display for Issue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(file) = &self.file {
            write!(f, "{}: ", file.display())?;
        }
        write!(f, "[{}]", self.recipe)?;
        if !self.location.is_empty() {
            write!(f, " {}", self.location)?;
        }
        write!(f, ": {}", self.kind)
    }
}
