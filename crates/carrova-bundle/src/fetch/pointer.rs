//! The release pointer: `recipes-latest/latest.json` = `{ "version": N }`.
//!
//! Recipe bundles are published as immutable `recipes-vN` releases. The
//! rolling `recipes-latest` release holds only this small pointer, so a
//! client never mixes assets of two bundles while the pointer is being
//! re-published: it reads N, then fetches every asset from `recipes-vN/`.

use serde::Deserialize;

use crate::{BundleError, BundleVersion, Result};

/// Path of the pointer below the release download base URL.
pub const LATEST_POINTER: &str = "recipes-latest/latest.json";

/// Largest accepted pointer file (bytes).
pub const POINTER_MAX_SIZE: u64 = 256;

/// Contents of the pointer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LatestPointer {
    /// Version of the current bundle (≥ 1).
    pub version: BundleVersion,
}

impl LatestPointer {
    /// Parse pointer bytes: exactly `{ "version": N }` with N ≥ 1, so N is
    /// a plain number before it ever becomes part of a URL.
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        let pointer: Self = serde_json::from_slice(bytes)
            .map_err(|e| BundleError::InvalidPointer(e.to_string()))?;
        if pointer.version.0 == 0 {
            return Err(BundleError::InvalidPointer(
                "version must be at least 1".into(),
            ));
        }
        Ok(pointer)
    }

    /// Tag of the immutable release holding this bundle: `recipes-vN`.
    #[must_use]
    pub fn release_tag(self) -> String {
        format!("recipes-v{}", self.version.0)
    }
}
