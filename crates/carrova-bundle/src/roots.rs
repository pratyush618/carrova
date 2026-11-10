//! Trusted root keys shipped with the engine (`keys/bundle-roots.json`).

use carrova_crypto::sign::TrustRoots;
use serde::Deserialize;

use crate::{BundleError, Result};

/// The repository's `keys/bundle-roots.json`, embedded at build time.
const RELEASE_ROOTS: &str = include_str!("../../../keys/bundle-roots.json");

/// Shape of a roots file: `{ "roots": ["<hex public key>", ...] }`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RootsFile {
    roots: TrustRoots,
}

/// Parse a roots file (`{ "roots": [...] }`, at least one key).
pub fn parse_roots(text: &str) -> Result<TrustRoots> {
    serde_json::from_str::<RootsFile>(text)
        .map(|f| f.roots)
        .map_err(BundleError::json("trust roots"))
}

/// Root keys this build trusts for recipe bundles.
pub fn release_roots() -> Result<TrustRoots> {
    parse_roots(RELEASE_ROOTS)
}
