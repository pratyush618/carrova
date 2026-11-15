//! Handing a URL or file to another device.

use serde::{Deserialize, Serialize};
use url::Url;

use crate::{FileOffer, LinkError};

/// Longest URL accepted, in bytes.
const MAX_URL: usize = 8 << 10;

/// What is being handed to the other device.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Handoff {
    /// A web link to open.
    Url(String),
    /// A file to receive.
    File(FileOffer),
}

/// Accepts only http(s) URLs with a host, at most 8 KiB.
pub fn check_url(url: &str) -> Result<(), LinkError> {
    if url.len() > MAX_URL {
        return Err(LinkError::TooBig {
            len: url.len(),
            max: MAX_URL,
        });
    }
    match Url::parse(url) {
        Ok(u) if matches!(u.scheme(), "http" | "https") && u.has_host() => Ok(()),
        _ => Err(LinkError::UnsafeUrl),
    }
}
