//! Handing a URL or file to another device.
//!
//! Wire on a `Handoff` lane (opener = sender), frames are length + CBOR:
//! sender → `Handoff`; receiver checks it, asks the user, → `OfferAnswer`.
//! A `File` handoff only announces the file: the bytes follow on a `File` lane.

use std::fmt;
use std::time::Duration;

use carrova_transport::{Connection, Stream};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::lane::{LaneKind, open_lane};
use crate::name::safe_file_name;
use crate::{ANSWER_TIMEOUT, FileOffer, LinkError, OfferAnswer};

/// How long the receiver waits for the sender's frame.
const FRAME_TIMEOUT: Duration = Duration::from_secs(30);

/// Longest URL accepted, in bytes.
const MAX_URL: usize = 8 << 10;

/// What is being handed to the other device. `Debug` hides the URL and file name.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Handoff {
    /// A web link to open.
    Url(String),
    /// A file to receive.
    File(FileOffer),
}

impl fmt::Debug for Handoff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Url(_) => "Handoff::Url(..)",
            Self::File(_) => "Handoff::File(..)",
        })
    }
}

/// A [`Handoff`] that passed the receiver's checks: only a [`SafeUrl`] can be
/// opened, and a file offer's name is already normalised. `Debug` hides both.
#[derive(Clone, PartialEq, Eq)]
pub enum ReceivedHandoff {
    /// A checked web link.
    Url(SafeUrl),
    /// A file offer with a [`safe_file_name`] name; the bytes follow on a `File` lane.
    File(FileOffer),
}

impl fmt::Debug for ReceivedHandoff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Url(_) => "ReceivedHandoff::Url(..)",
            Self::File(_) => "ReceivedHandoff::File(..)",
        })
    }
}

/// A URL that passed [`check_url`], in the parser's normalised form: the only
/// form that may be shown to the user or opened. `Debug` hides it.
#[derive(Clone, PartialEq, Eq)]
pub struct SafeUrl(String);

impl fmt::Debug for SafeUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SafeUrl(..)")
    }
}

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

/// Offer `handoff` on a new `Handoff` lane and wait up to [`ANSWER_TIMEOUT`]
/// (the receiver's 60 s ask plus margin) for the user's answer.
pub fn send_handoff(conn: &Connection, handoff: &Handoff) -> Result<OfferAnswer, LinkError> {
    if let Handoff::Url(url) = handoff {
        check_url(url)?;
    }
    let mut stream = open_lane(conn, LaneKind::Handoff)?;
    stream.send(handoff)?;
    Ok(stream.recv(ANSWER_TIMEOUT)?)
}

/// Receive a handoff on an accepted `Handoff` lane. A URL must pass
/// [`check_url`] and a file name is normalised with [`safe_file_name`] before
/// `decide` sees it; `decide` answers the sender. `Some` only if it accepted.
pub fn recv_handoff(
    mut stream: Stream,
    decide: &mut dyn FnMut(&ReceivedHandoff) -> OfferAnswer,
) -> Result<Option<ReceivedHandoff>, LinkError> {
    let received = match stream.recv(FRAME_TIMEOUT)? {
        Handoff::Url(url) => ReceivedHandoff::Url(check_url(&url)?),
        Handoff::File(mut offer) => {
            offer.name = safe_file_name(&offer.name);
            ReceivedHandoff::File(offer)
        }
    };
    let answer = decide(&received);
    stream.send(&answer)?;
    stream.finish()?;
    Ok(matches!(answer, OfferAnswer::Accept { .. }).then_some(received))
}
