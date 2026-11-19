//! Release downloads over HTTPS with `ureq` (rustls, blocking): redirects
//! are followed by hand, only to the hosts releases come from.

use std::io::Read;
use std::time::Duration;

use carrova_view::ipc::ErrorCode;
use carrova_view::update::apk::trusted_url;
use ureq::Agent;
use ureq::http::{Response, StatusCode, header};

use crate::apk_download::{Body, Source, network};
use crate::command_error::CommandError;

/// Time allowed to connect.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
/// Time allowed without a byte arriving, so a stalled link fails.
const READ_TIMEOUT: Duration = Duration::from_secs(20);
/// Redirects followed (GitHub sends one, to its asset storage).
const MAX_REDIRECTS: usize = 5;

/// Fetches release files from GitHub.
#[derive(Debug)]
pub struct Https(Agent);

impl Default for Https {
    fn default() -> Self {
        let agent = Agent::config_builder()
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

impl Https {
    /// The whole file at `url`, at most `limit` bytes.
    ///
    /// # Errors
    ///
    /// [`ErrorCode::Network`] if it cannot be fetched or is larger.
    pub fn get(&self, url: &str, limit: u64) -> Result<Vec<u8>, CommandError> {
        let mut bytes = Vec::new();
        self.open(url, 0)?
            .reader
            .take(limit.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|e| network(&e))?;
        if bytes.len() as u64 > limit {
            return Err(failed("the release feed is too large"));
        }
        Ok(bytes)
    }

    /// GET `url` (from byte `offset`), following trusted redirects.
    fn call(&self, url: &str, offset: u64) -> Result<Response<ureq::Body>, CommandError> {
        let mut url = url.to_owned();
        for _ in 0..=MAX_REDIRECTS {
            if !trusted_url(&url) {
                return Err(failed("the download left GitHub"));
            }
            let mut request = self.0.get(&url);
            if offset > 0 {
                request = request.header(header::RANGE, format!("bytes={offset}-"));
            }
            let response = request.call().map_err(|e| failed(&e.to_string()))?;
            if !response.status().is_redirection() {
                return Ok(response);
            }
            url = response
                .headers()
                .get(header::LOCATION)
                .and_then(|location| location.to_str().ok())
                .ok_or_else(|| failed("a redirect without a location"))?
                .to_owned();
        }
        Err(failed("too many redirects"))
    }
}

impl Source for Https {
    fn open(&self, url: &str, offset: u64) -> Result<Body, CommandError> {
        let response = self.call(url, offset)?;
        let length = response
            .headers()
            .get(header::CONTENT_LENGTH)
            .and_then(|len| len.to_str().ok()?.parse::<u64>().ok());
        let start = match response.status() {
            StatusCode::PARTIAL_CONTENT => offset,
            StatusCode::OK => 0,
            // The partial file no longer fits the server's: start over.
            StatusCode::RANGE_NOT_SATISFIABLE if offset > 0 => return self.open(url, 0),
            StatusCode::NOT_FOUND => {
                return Err(failed("this release has no Android download yet"));
            }
            status => return Err(failed(&format!("the server answered {status}"))),
        };
        Ok(Body {
            reader: Box::new(response.into_body().into_reader()),
            start,
            total: length.map(|len| start + len),
        })
    }
}

fn failed(message: &str) -> CommandError {
    CommandError::new(ErrorCode::Network, message)
        .with_hint("Check your network connection and try again.")
}
