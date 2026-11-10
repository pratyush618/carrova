use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{ManifestError, Result};

/// A validated relative path below a semantic root.
///
/// Invariants (checked on construction **and** on decode):
/// `/`-separated, non-empty, no leading `/`, no empty / `.` / `..`
/// components, no control characters (C0 incl. NUL, DEL, C1) and no
/// Unicode bidi controls (see [`is_unsafe_char`](crate::is_unsafe_char)).
/// Receivers rely on this; nothing is normalised silently except that
/// [`RelPath::new`] strips leading `./` prefixes.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RelPath(String);

impl RelPath {
    /// Validate `path`, stripping any leading `./` first.
    pub fn new(path: impl Into<String>) -> Result<Self> {
        let path = path.into();
        let mut s = path.as_str();
        while let Some(rest) = s.strip_prefix("./") {
            s = rest;
        }
        Self::strict(s.to_owned())
    }

    /// Validate without any stripping (used on decode).
    fn strict(path: String) -> Result<Self> {
        let err = |reason| {
            Err(ManifestError::InvalidRelPath {
                path: crate::text::escaped(&path),
                reason,
            })
        };
        if path.is_empty() {
            return err("empty path");
        }
        if path.starts_with('/') {
            return err("absolute path");
        }
        if path.contains('\0') {
            return err("contains NUL byte");
        }
        if path.chars().any(crate::is_unsafe_char) {
            return err("contains a control or bidi character");
        }
        for comp in path.split('/') {
            match comp {
                "" => return err("empty path component"),
                "." => return err("`.` component"),
                ".." => return err("`..` component"),
                _ => {}
            }
        }
        Ok(Self(path))
    }

    /// The path as a `/`-separated string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Path components, in order.
    pub fn components(&self) -> impl Iterator<Item = &str> {
        self.0.split('/')
    }
}

impl TryFrom<String> for RelPath {
    type Error = ManifestError;

    fn try_from(s: String) -> Result<Self> {
        Self::strict(s)
    }
}

impl From<RelPath> for String {
    fn from(p: RelPath) -> Self {
        p.0
    }
}

impl fmt::Display for RelPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
