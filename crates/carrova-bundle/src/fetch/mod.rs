//! Fetching bundle assets: the [`Fetcher`] trait and its two
//! implementations.

use std::io::Read;

use crate::{Asset, BundleVersion, FetchError};

mod dir;
mod http;
mod pointer;

pub use dir::DirFetcher;
pub use http::{DEFAULT_BASE_URL, HttpConfig, HttpFetcher};
pub use pointer::{LATEST_POINTER, LatestPointer, POINTER_MAX_SIZE};

/// Source of bundle assets (a release URL, a directory…).
pub trait Fetcher: std::fmt::Debug {
    /// Bytes of `asset`, failing with [`FetchError::TooLarge`] beyond
    /// `limit` bytes.
    fn get(&self, asset: &Asset, limit: u64) -> Result<Vec<u8>, FetchError>;

    /// The bundle version this source is known to hold (the release the
    /// [`LatestPointer`] named), if any. [`crate::Updater::check`] refuses
    /// an index of any other version.
    fn release(&self) -> Option<BundleVersion> {
        None
    }
}

/// Borrowed fetchers fetch too (e.g. to inspect a fetcher after use).
impl<F: Fetcher + ?Sized> Fetcher for &F {
    fn get(&self, asset: &Asset, limit: u64) -> Result<Vec<u8>, FetchError> {
        (**self).get(asset, limit)
    }

    fn release(&self) -> Option<BundleVersion> {
        (**self).release()
    }
}

/// Read all of `reader`, failing once more than `limit` bytes arrive.
fn read_capped(reader: impl Read, limit: u64) -> Result<Vec<u8>, FetchError> {
    let mut out = Vec::new();
    reader.take(limit.saturating_add(1)).read_to_end(&mut out)?;
    if out.len() as u64 > limit {
        return Err(FetchError::TooLarge { limit });
    }
    Ok(out)
}
