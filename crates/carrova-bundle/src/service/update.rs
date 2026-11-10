//! One update attempt, as every front-end runs it.

use carrova_crypto::sign::TrustRoots;
use carrova_platform::Platform;

use crate::{
    BundleError, BundleStore, BundleVersion, FetchError, Fetcher, HttpConfig, HttpFetcher,
    InstallOutcome, Result, UpdateOutcome, Updater, now_unix,
};

/// How an update attempt ended, when it did not fail.
#[derive(Debug, Clone)]
pub enum UpdateAttempt {
    /// The feed answered: see the outcome.
    Done(UpdateOutcome),
    /// The release pointer or an asset is missing (HTTP 404): a new
    /// bundle is most likely being published right now. Nothing changed;
    /// try again in a minute.
    RetryLater,
}

impl UpdateAttempt {
    /// The retry policy, in one place: a missing asset means "being
    /// published", every other failure is an error.
    fn judge(result: Result<UpdateOutcome>) -> Result<Self> {
        match result {
            Ok(outcome) => Ok(Self::Done(outcome)),
            Err(BundleError::Fetch {
                source: FetchError::NotFound,
                ..
            }) => {
                tracing::info!("recipe bundle not published (yet); retry later");
                Ok(Self::RetryLater)
            }
            Err(e) => Err(e),
        }
    }
}

impl UpdateOutcome {
    /// The version this update newly installed; `None` if nothing changed.
    #[must_use]
    pub fn installed(&self) -> Option<BundleVersion> {
        match self {
            Self::Installed {
                outcome: InstallOutcome::Installed { version, .. },
                ..
            } => Some(*version),
            Self::Installed {
                outcome: InstallOutcome::AlreadyCurrent(_),
                ..
            }
            | Self::UpToDate(_)
            | Self::Refused(_) => None,
        }
    }
}

/// Check the release feed at `http` for a bundle newer than the one in
/// `platform`'s store, verify it against `roots` now ([`now_unix`]) and
/// install it.
///
/// # Errors
///
/// The store cannot be opened, the URL is unusable, or download,
/// verification or install fails. A missing bundle is
/// [`UpdateAttempt::RetryLater`], not an error.
pub fn update(
    platform: &dyn Platform,
    roots: TrustRoots,
    http: &HttpConfig,
) -> Result<UpdateAttempt> {
    let store = BundleStore::open(platform, roots.clone())?;
    let now = now_unix()?;
    match HttpFetcher::latest(http) {
        Ok(fetcher) => update_with(fetcher, roots, &store, now),
        Err(e) => UpdateAttempt::judge(Err(e)),
    }
}

/// [`update`] from any `fetcher` into `store`, verified at `now` (unix
/// seconds): the seam tests and local bundles use.
///
/// # Errors
///
/// See [`update`].
pub fn update_with(
    fetcher: impl Fetcher,
    roots: TrustRoots,
    store: &BundleStore<'_>,
    now: u64,
) -> Result<UpdateAttempt> {
    UpdateAttempt::judge(Updater::new(fetcher, roots).update(store, now))
}
