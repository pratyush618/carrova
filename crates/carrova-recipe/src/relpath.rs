//! [`RelPath`]: a path relative to a semantic root, confined by construction.

use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Why a string is not an acceptable relative path.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RelPathError {
    #[error("relative path is empty")]
    Empty,
    #[error("relative path contains a NUL byte")]
    Nul,
    #[error("relative path must not be absolute (leading `/`): {0:?}")]
    Absolute(String),
    #[error("relative path must not start with `~`: {0:?}")]
    Tilde(String),
    #[error("relative path must use `/`, not `\\`: {0:?}")]
    Backslash(String),
    #[error("relative path must not start with a drive letter: {0:?}")]
    DriveLetter(String),
    #[error("relative path must not contain `..`: {0:?}")]
    ParentComponent(String),
}

/// Normalised, confined path relative to a [`crate::SemanticRoot`].
///
/// Invariants (checked on construction and on deserialize):
/// non-empty, `/`-separated, no leading `/` or `~`, no `\`, no NUL, no drive
/// letter, no `..` component. `.` components and repeated `/` are dropped, so
/// `./git//config` becomes `git/config`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RelPath(String);

impl RelPath {
    /// Validate and normalise `raw`.
    pub fn new(raw: &str) -> Result<Self, RelPathError> {
        if raw.is_empty() {
            return Err(RelPathError::Empty);
        }
        if raw.contains('\0') {
            return Err(RelPathError::Nul);
        }
        if raw.starts_with('/') {
            return Err(RelPathError::Absolute(raw.to_owned()));
        }
        if raw.starts_with('~') {
            return Err(RelPathError::Tilde(raw.to_owned()));
        }
        if raw.contains('\\') {
            return Err(RelPathError::Backslash(raw.to_owned()));
        }
        let b = raw.as_bytes();
        if b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':' {
            return Err(RelPathError::DriveLetter(raw.to_owned()));
        }
        let mut parts = Vec::new();
        for comp in raw.split('/') {
            match comp {
                "" | "." => {}
                ".." => return Err(RelPathError::ParentComponent(raw.to_owned())),
                c => parts.push(c),
            }
        }
        if parts.is_empty() {
            return Err(RelPathError::Empty);
        }
        Ok(Self(parts.join("/")))
    }

    /// Normalised string form (`/`-separated).
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Path components, in order.
    pub fn components(&self) -> impl Iterator<Item = &str> {
        self.0.split('/')
    }

    /// `base` joined with every component (OS-native separators).
    #[must_use]
    pub fn join_to(&self, base: &Path) -> PathBuf {
        let mut p = base.to_path_buf();
        for c in self.components() {
            p.push(c);
        }
        p
    }
}

impl TryFrom<String> for RelPath {
    type Error = RelPathError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(&value)
    }
}

impl From<RelPath> for String {
    fn from(value: RelPath) -> Self {
        value.0
    }
}

impl fmt::Display for RelPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
