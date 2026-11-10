//! [`PluginId`]: the validated name of a WASM plugin a recipe item may use.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// Why a string is not a [`PluginId`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PluginIdError {
    /// The id is empty.
    #[error("plugin id is empty")]
    Empty,
    /// The id is longer than [`PluginId::MAX_LEN`] bytes.
    #[error("plugin id is {len} characters long (at most {max})", max = PluginId::MAX_LEN)]
    TooLong {
        /// Length of the rejected id, in bytes.
        len: usize,
    },
    /// The id contains a character outside `[a-z0-9-]`.
    #[error("plugin id `{id}` may only contain `a-z`, `0-9` and `-`")]
    InvalidChar {
        /// The rejected id.
        id: String,
    },
}

/// Plugin identifier: 1 to [`PluginId::MAX_LEN`] characters of `[a-z0-9-]`.
///
/// Checked on construction and on deserialize, so a held `PluginId` is
/// always safe to use as a file name or lookup key.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct PluginId(String);

impl PluginId {
    /// Longest accepted id, in bytes.
    pub const MAX_LEN: usize = 64;

    /// Validate `raw` (no normalisation: what is accepted is kept verbatim).
    pub fn new(raw: &str) -> Result<Self, PluginIdError> {
        Self::check(raw)?;
        Ok(Self(raw.to_owned()))
    }

    /// The id as written.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn check(raw: &str) -> Result<(), PluginIdError> {
        if raw.is_empty() {
            return Err(PluginIdError::Empty);
        }
        if raw.len() > Self::MAX_LEN {
            return Err(PluginIdError::TooLong { len: raw.len() });
        }
        if !raw
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        {
            return Err(PluginIdError::InvalidChar { id: raw.to_owned() });
        }
        Ok(())
    }
}

impl FromStr for PluginId {
    type Err = PluginIdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

impl TryFrom<String> for PluginId {
    type Error = PluginIdError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::check(&value)?;
        Ok(Self(value))
    }
}

impl From<PluginId> for String {
    fn from(value: PluginId) -> Self {
        value.0
    }
}

impl fmt::Display for PluginId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
