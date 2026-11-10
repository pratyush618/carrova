//! Assets over HTTP(S) with `ureq` (rustls, blocking, no async runtime).

use std::time::Duration;

use super::{Fetcher, LATEST_POINTER, LatestPointer, POINTER_MAX_SIZE, read_capped};
use crate::{Asset, BundleError, BundleVersion, FetchError};

/// The release download base URL clients update from
/// ([`HttpFetcher::latest`]): `<base>/recipes-latest/latest.json` names the
/// current bundle, whose assets live in the immutable `<base>/recipes-vN/`
/// release. Shared by the CLI and the desktop app.
///
/// **Placeholder**: replace `pratyush618/carrova` with the real GitHub repository
/// before the first release (user action).
pub const DEFAULT_BASE_URL: &str = "https://github.com/pratyush618/carrova/releases/download/";

/// Settings for [`HttpFetcher`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpConfig {
    /// Base URL: for [`HttpFetcher::new`] the directory holding the assets
    /// (e.g. `…/releases/download/recipes-v7`); for
    /// [`HttpFetcher::latest`] the release download base
    /// ([`DEFAULT_BASE_URL`]).
    pub base_url: String,
    /// Time allowed to connect.
    pub connect_timeout: Duration,
    /// Time allowed for a whole request, redirects and body included.
    pub request_timeout: Duration,
    /// Refuse plain `http://` (also on redirects). Signatures protect
    /// integrity either way; HTTPS adds privacy.
    pub require_https: bool,
}

impl HttpConfig {
    /// Defaults for `base_url`: 10 s connect, 60 s per request, HTTPS only.
    #[must_use]
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            connect_timeout: Duration::from_secs(10),
            request_timeout: Duration::from_secs(60),
            require_https: true,
        }
    }
}

/// Fetches `<base_url>/<asset file name>`.
#[derive(Debug)]
pub struct HttpFetcher {
    base: String,
    agent: ureq::Agent,
    release: Option<BundleVersion>,
}

impl HttpFetcher {
    /// Fetcher for `config`; fails if HTTPS is required but the URL is not
    /// `https://`.
    pub fn new(config: &HttpConfig) -> Result<Self, FetchError> {
        let base = config.base_url.trim_end_matches('/').to_owned();
        if config.require_https && !base.starts_with("https://") {
            return Err(FetchError::Config(format!("{base} is not an https:// URL")));
        }
        let agent = ureq::Agent::config_builder()
            .timeout_connect(Some(config.connect_timeout))
            .timeout_global(Some(config.request_timeout))
            .https_only(config.require_https)
            .build()
            .into();
        Ok(Self {
            base,
            agent,
            release: None,
        })
    }

    /// Fetcher for the current bundle under the release download base
    /// `config.base_url` ([`DEFAULT_BASE_URL`]): reads the
    /// [`LatestPointer`] (`recipes-latest/latest.json`, at most
    /// [`POINTER_MAX_SIZE`] bytes) and fetches every asset from the
    /// immutable `recipes-vN/` release it names. A missing pointer is
    /// [`FetchError::NotFound`] (being re-published: retry later).
    pub fn latest(config: &HttpConfig) -> Result<Self, BundleError> {
        let feed = Self::new(config).map_err(|source| BundleError::Fetch {
            name: LATEST_POINTER.to_owned(),
            source,
        })?;
        let bytes = feed
            .get_path(LATEST_POINTER, POINTER_MAX_SIZE)
            .map_err(|source| BundleError::Fetch {
                name: LATEST_POINTER.to_owned(),
                source,
            })?;
        let pointer = LatestPointer::parse(&bytes)?;
        tracing::debug!(version = %pointer.version, "release pointer read");
        Ok(Self {
            base: format!("{}/{}", feed.base, pointer.release_tag()),
            agent: feed.agent,
            release: Some(pointer.version),
        })
    }

    /// GET `<base>/<path>`, at most `limit` bytes.
    fn get_path(&self, path: &str, limit: u64) -> Result<Vec<u8>, FetchError> {
        let url = format!("{}/{path}", self.base);
        let response = self.agent.get(&url).call().map_err(|e| match e {
            ureq::Error::StatusCode(404) => FetchError::NotFound,
            other => FetchError::Transport(other.to_string()),
        })?;
        read_capped(response.into_body().into_reader(), limit)
    }
}

impl Fetcher for HttpFetcher {
    fn get(&self, asset: &Asset, limit: u64) -> Result<Vec<u8>, FetchError> {
        self.get_path(&asset.file_name(), limit)
    }

    fn release(&self) -> Option<BundleVersion> {
        self.release
    }
}
