//! One update attempt, as every front-end runs it.

use carrova_crypto::sign::TrustRoots;
use carrova_platform::Platform;

use crate::{
    BundleError, BundleStore, BundleVersion, FetchError, Fetcher, HttpConfig, HttpFetcher,
    InstallOutcome, Result, UpdateCheck, UpdateOutcome, Updater, now_unix,
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

/// How an update check ([`check`]) ended, when it did not fail.
#[derive(Debug, Clone)]
pub enum CheckAttempt {
    /// The feed answered: see the check.
    Done(UpdateCheck),
    /// The bundle is not published (yet): see [`UpdateAttempt::RetryLater`].
    RetryLater,
}

/// The retry policy, in one place: a missing asset (`Ok(None)`) means
/// "being published", every other failure is an error.
fn judge<T>(result: Result<T>) -> Result<Option<T>> {
    match result {
        Ok(done) => Ok(Some(done)),
        Err(BundleError::Fetch {
            source: FetchError::NotFound,
            ..
        }) => {
            tracing::info!("recipe bundle not published (yet); retry later");
            Ok(None)
        }
        Err(e) => Err(e),
    }
}

impl UpdateAttempt {
    fn judge(result: Result<UpdateOutcome>) -> Result<Self> {
        Ok(judge(result)?.map_or(Self::RetryLater, Self::Done))
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

/// Whether the release feed at `http` offers a bundle newer than the one
/// in `platform`'s store, verified against `roots` now; installs nothing.
///
/// # Errors
///
/// As [`update`], minus the install.
pub fn check(
    platform: &dyn Platform,
    roots: TrustRoots,
    http: &HttpConfig,
) -> Result<CheckAttempt> {
    let store = BundleStore::open(platform, roots.clone())?;
    let now = now_unix()?;
    let checked = HttpFetcher::latest(http)
        .and_then(|fetcher| Updater::new(fetcher, roots).check(&store, now));
    Ok(judge(checked)?.map_or(CheckAttempt::RetryLater, CheckAttempt::Done))
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
