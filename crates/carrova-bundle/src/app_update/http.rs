//! Release downloads over HTTPS: redirects followed by hand, only to the
//! hosts releases come from, and resumes that are proven to continue the
//! same file (`If-Range` on its `ETag`, `Content-Range` from the offset
//! asked, with the file's size) or start over. [`Get`] is the one
//! network seam; [`Https`] is the real one (`ureq`, rustls, blocking).

use std::io::{self, Read};
use std::time::Duration;

use super::{AppUpdateError, FEED_MAX_SIZE, FEED_URL, NetFailure, Release};

/// Hosts a release download may come from, redirects included: GitHub and
/// its release asset storage.
const TRUSTED_HOSTS: [&str; 3] = [
    "github.com",
    "objects.githubusercontent.com",
    "release-assets.githubusercontent.com",
];
/// Redirects followed (GitHub sends one, to its asset storage).
const MAX_REDIRECTS: usize = 5;
/// Time allowed to connect.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
/// Time allowed without a byte arriving, so a stalled link fails.
const READ_TIMEOUT: Duration = Duration::from_secs(20);

/// Where a partial download stands: `offset` bytes of the file whose
/// `ETag` was `etag`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resume {
    /// Bytes already on disk.
    pub offset: u64,
    /// The file's `ETag` when they were fetched.
    pub etag: String,
}

/// One HTTP answer, as far as a download needs it.
pub struct Reply {
    /// HTTP status.
    pub status: u16,
    /// `Location`, on a redirect.
    pub location: Option<String>,
    /// `ETag`.
    pub etag: Option<String>,
    /// `Content-Range`, on a partial answer.
    pub content_range: Option<String>,
    /// `Content-Length`.
    pub content_length: Option<u64>,
    /// The body.
    pub body: Box<dyn Read + Send>,
}

impl std::fmt::Debug for Reply {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Reply")
            .field("status", &self.status)
            .finish_non_exhaustive()
    }
}

/// One GET, redirects not followed.
pub trait Get {
    /// GET `url`; with `resume`, only the rest of the file, if it is still
    /// the same one (`Range` + `If-Range`).
    ///
    /// # Errors
    ///
    /// [`NetFailure`] if no answer arrives.
    fn get(&self, url: &str, resume: Option<&Resume>) -> Result<Reply, NetFailure>;
}

/// A body from some offset on.
pub(super) struct Body {
    pub(super) reader: Box<dyn Read + Send>,
    /// Where it starts: the offset resumed from, or 0 (the whole file).
    pub(super) start: u64,
    /// Size of the whole file, if the server said.
    pub(super) total: Option<u64>,
    /// The file's `ETag`, if any (needed to resume it later).
    pub(super) etag: Option<String>,
}

/// Whether `url` is an HTTPS URL on a host releases come from (no port,
/// no user info).
#[must_use]
pub fn trusted_url(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else {
        return false;
    };
    let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
    TRUSTED_HOSTS.contains(&host.to_ascii_lowercase().as_str())
}

/// The newest release, from the feed.
///
/// # Errors
///
/// [`AppUpdateError::Network`] if the feed cannot be fetched (or is too
/// large), [`AppUpdateError::Feed`] if it is not valid.
pub fn fetch_release(get: &impl Get) -> Result<Release, AppUpdateError> {
    let reply = follow(get, FEED_URL, None)?;
    if reply.status != 200 {
        return Err(NetFailure::Status(reply.status).into());
    }
    let mut bytes = Vec::new();
    reply
        .body
        .take(FEED_MAX_SIZE.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|e| NetFailure::Read(e.kind()))?;
    if bytes.len() as u64 > FEED_MAX_SIZE {
        return Err(AppUpdateError::TooLarge);
    }
    Ok(Release::parse(&bytes)?)
}

/// The file at `url`, from where `resume` stands if the server proves it
/// is still the same file; otherwise the whole of it.
pub(super) fn open(get: &impl Get, url: &str, resume: Option<&Resume>) -> Result<Body, NetFailure> {
    let reply = follow(get, url, resume)?;
    match (reply.status, resume) {
        (206, Some(resume)) => match content_range(reply.content_range.as_deref()) {
            Some((start, Some(total))) if start == resume.offset => Ok(Body {
                reader: reply.body,
                start,
                total: Some(total),
                etag: reply.etag.or_else(|| Some(resume.etag.clone())),
            }),
            // Not the bytes asked for, or no size to tell when the file
            // ends (the range may stop short): the partial file cannot be
            // trusted.
            _ => open(get, url, None),
        },
        (200, _) => Ok(Body {
            reader: reply.body,
            start: 0,
            total: reply.content_length,
            etag: reply.etag,
        }),
        // The partial file no longer fits the server's: start over.
        (416, Some(_)) => open(get, url, None),
        (404, _) => Err(NetFailure::Missing),
        (status, _) => Err(NetFailure::Status(status)),
    }
}

/// GET `url`, following redirects only to trusted hosts.
fn follow(get: &impl Get, url: &str, resume: Option<&Resume>) -> Result<Reply, NetFailure> {
    let mut url = url.to_owned();
    for _ in 0..=MAX_REDIRECTS {
        if !trusted_url(&url) {
            return Err(NetFailure::Untrusted);
        }
        let reply = get.get(&url, resume)?;
        if !(300..400).contains(&reply.status) {
            return Ok(reply);
        }
        url = reply.location.ok_or(NetFailure::Redirects)?;
    }
    Err(NetFailure::Redirects)
}

/// `start` and `total` of `bytes start-end/total` (`total` may be `*`).
fn content_range(header: Option<&str>) -> Option<(u64, Option<u64>)> {
    let (range, total) = header?.strip_prefix("bytes ")?.split_once('/')?;
    let start = range.split_once('-')?.0.trim().parse().ok()?;
    Some((start, total.trim().parse().ok()))
}

/// Release downloads with `ureq`: HTTPS only, statuses and redirects left
/// to the caller (see the module docs).
#[derive(Debug)]
pub struct Https(ureq::Agent);

impl Default for Https {
    fn default() -> Self {
        let agent = ureq::Agent::config_builder()
            .https_only(true)
            .max_redirects(0)
            .http_status_as_error(false)
            .timeout_connect(Some(CONNECT_TIMEOUT))
            .timeout_per_read(Some(READ_TIMEOUT))
            .build()
            .into();
        Self(agent)
    }
}

impl Get for Https {
    fn get(&self, url: &str, resume: Option<&Resume>) -> Result<Reply, NetFailure> {
        let mut request = self.0.get(url);
        if let Some(resume) = resume {
            request = request
                .header("Range", format!("bytes={}-", resume.offset))
                .header("If-Range", resume.etag.as_str());
        }
        let response = request.call().map_err(|e| net_failure(&e))?;
        let header = |name: &str| {
            response
                .headers()
                .get(name)
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned)
        };
        Ok(Reply {
            status: response.status().as_u16(),
            location: header("location"),
            etag: header("etag"),
            content_range: header("content-range"),
            content_length: header("content-length").and_then(|len| len.parse().ok()),
            body: Box::new(response.into_body().into_reader()),
        })
    }
}

/// A `ureq` failure by kind only: its message can hold the URL.
fn net_failure(error: &ureq::Error) -> NetFailure {
    match error {
        ureq::Error::Timeout(_) => NetFailure::Timeout,
        ureq::Error::Io(e) if e.kind() == io::ErrorKind::TimedOut => NetFailure::Timeout,
        ureq::Error::Io(e) => NetFailure::Read(e.kind()),
        _ => NetFailure::Connect,
    }
}

#[cfg(test)]
#[path = "../../tests/app_update/http/unit.rs"]
mod tests;
