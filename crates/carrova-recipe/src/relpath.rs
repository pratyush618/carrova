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
    #[error(
        "relative path must not be absolute (leading separator): {0:?}; use a semantic root, e.g. `root: user_config` with a relative path"
    )]
    Absolute(String),
    #[error("relative path must not start with `~`: {0:?}")]
    Tilde(String),
    #[error("relative path must use `/`, not `\\`: {0:?}")]
    Backslash(String),
    /// A `:` anywhere: a drive letter (`C:`, also mid-path, where Windows
    /// `PathBuf::push` would replace the base) or an NTFS stream.
    #[error(
        "relative path must not contain `:` (drive letter): {0:?}; use a semantic root, e.g. `root: user_config` with a relative path"
    )]
    Colon(String),
    #[error("relative path must not contain `..`: {0:?}")]
    ParentComponent(String),
}

/// Normalised, confined path relative to a [`crate::SemanticRoot`].
///
/// Invariants (checked on construction and on deserialize):
/// non-empty, `/`-separated, no leading `/` or `~`, no `\`, no NUL, no `:`
/// (drive letter) in any component, no `..` component. `.` components and
/// repeated `/` are dropped, so `./git//config` becomes `git/config`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RelPath(String);

impl RelPath {
    /// Validate and normalise `raw` (`/`-separated; `\` is rejected because
    /// it is a valid filename character on Linux and macOS).
    pub fn new(raw: &str) -> Result<Self, RelPathError> {
        Self::parse(raw, &['/'], false)
    }

    /// Like [`RelPath::new`], but allows `:` in names. Only for paths read
    /// from the sending computer's own disk (raw items), where `:` is a
    /// valid Linux/macOS name character and can't occur on Windows; the
    /// receiver still refuses names its OS can't hold.
    pub fn new_raw(raw: &str) -> Result<Self, RelPathError> {
        Self::parse(raw, &['/'], true)
    }

    /// Like [`RelPath::new`], but also accepts Windows-native `\` separators,
    /// normalised to `/`. Only for paths that exist solely on Windows (a
    /// recipe's `os.windows` block). Drive letters (anywhere), UNC and rooted
    /// paths stay rejected.
    pub fn new_windows(raw: &str) -> Result<Self, RelPathError> {
        Self::parse(raw, &['/', '\\'], false)
    }

    fn parse(raw: &str, seps: &[char], allow_colon: bool) -> Result<Self, RelPathError> {
        if raw.is_empty() {
            return Err(RelPathError::Empty);
        }
        if raw.contains('\0') {
            return Err(RelPathError::Nul);
        }
        if raw.starts_with(seps) {
            return Err(RelPathError::Absolute(raw.to_owned()));
        }
        if raw.starts_with('~') {
            return Err(RelPathError::Tilde(raw.to_owned()));
        }
        if !seps.contains(&'\\') && raw.contains('\\') {
            return Err(RelPathError::Backslash(raw.to_owned()));
        }
        if !allow_colon && raw.contains(':') {
            return Err(RelPathError::Colon(raw.to_owned()));
        }
        let mut parts = Vec::new();
        for comp in raw.split(seps) {
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
