//! The wall clock signatures are checked against.

use std::time::{SystemTime, UNIX_EPOCH};

use crate::{BundleError, Result};

/// Seconds since the Unix epoch: the time remote bundles and signer
/// certificates are verified at ([`crate::VerifyTime::Now`]).
///
/// # Errors
///
/// [`BundleError::Clock`] if the system clock is before 1970 (a
/// certificate check at a bogus time would be meaningless).
pub fn now_unix() -> Result<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .map_err(|_| BundleError::Clock)
}
