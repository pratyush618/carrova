//! The bundle shipped inside an app or packaged CLI, and its build-time pin.
//!
//! Packaging (`just package`) exports `CARROVA_SHIPPED_INDEX_HASH`, the
//! BLAKE3 of the shipped `index.json` (`cargo xtask bundle pin`), while
//! compiling this crate; both front-ends link it, so both get the same
//! pin. At runtime the shipped index must then hash to exactly that value
//! **and** carry a signature chain that was valid when it was signed
//! ([`VerifyTime::SignedAt`]): the app may outlive the short-lived signer
//! certificate, but nobody can swap in another validly signed bundle.
//!
//! Builds without the variable (debug and dev builds, `cargo install`)
//! have no pin and accept a shipped bundle by its signature chain alone.

use carrova_crypto::sign::TrustRoots;

use crate::hash::is_hash_text;
use crate::{
    BlobHash, BundleError, Catalog, Fetcher, Result, SignedIndex, VerifyTime, open_bundle,
};

/// Name of the build-time variable holding the pin.
pub const SHIPPED_INDEX_HASH_ENV: &str = "CARROVA_SHIPPED_INDEX_HASH";

/// The pin as compiled in (cargo rebuilds this crate when it changes).
const PIN: Option<&str> = option_env!("CARROVA_SHIPPED_INDEX_HASH");

// A malformed pin fails the build instead of silently disabling pinning.
const _: () = assert!(
    match PIN {
        Some(pin) => is_hash_text(pin),
        None => true,
    },
    "CARROVA_SHIPPED_INDEX_HASH must be 64 lowercase hex digits"
);

/// BLAKE3 of the shipped `index.json` pinned into this build, if any.
#[must_use]
pub fn shipped_index_pin() -> Option<BlobHash> {
    // Checked at compile time above, so parsing cannot fail.
    PIN.and_then(|pin| pin.parse().ok())
}

/// Open the shipped bundle served by `fetcher`: its chain is verified at
/// signing time and, with a `pin`, its index must hash to it.
pub(crate) fn open_shipped(
    fetcher: &dyn Fetcher,
    roots: &TrustRoots,
    pin: Option<BlobHash>,
) -> Result<(SignedIndex, Catalog)> {
    let (signed, catalog) = open_bundle(fetcher, roots, VerifyTime::SignedAt)?;
    if let Some(expected) = pin {
        let found = BlobHash::of(signed.index_bytes());
        if found != expected {
            return Err(BundleError::ShippedPinMismatch { expected, found });
        }
    }
    Ok((signed, catalog))
}
