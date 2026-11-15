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

/// A URL that passed [`check_url`], in the parser's normalised form: the only
/// form that may be shown to the user or opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SafeUrl(String);

impl SafeUrl {
    /// The normalised URL.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Accepts only plain `http(s)://host...` URLs of at most 8 KiB, with no
/// credentials, whitespace or control characters, and returns the normalised
/// form. Inputs the parser would silently rewrite (`http:/x`, padded, ...) are refused.
pub fn check_url(url: &str) -> Result<SafeUrl, LinkError> {
    if url.len() > MAX_URL {
        return Err(LinkError::TooBig {
            len: url.len(),
            max: MAX_URL,
        });
    }
    let plain = url.chars().all(|c| !c.is_whitespace() && !c.is_control());
    // The parser rewrites `\` to `/` and skips extra slashes after the scheme.
    let http = !url.contains('\\')
        && ["http://", "https://"].iter().any(|p| {
            url.get(..p.len())
                .is_some_and(|s| s.eq_ignore_ascii_case(p))
                && !url[p.len()..].starts_with('/')
        });
    match Url::parse(url) {
        Ok(u)
            if plain
                && http
                && u.has_host()
                && u.username().is_empty()
                && u.password().is_none() =>
        {
            Ok(SafeUrl(u.into()))
        }
        _ => Err(LinkError::UnsafeUrl),
    }
}
