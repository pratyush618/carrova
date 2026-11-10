//! Client update flow: check → fetch (delta) → install.

use carrova_crypto::sign::TrustRoots;

use crate::catalog::{fetch_blob, fetch_signed};
use crate::policy::{Admission, Baseline};
use crate::{
    BundleError, BundleStore, BundleVersion, Fetcher, InstallOutcome, RecipeEntry, Result,
    SignedIndex, SkippedRecipe, VerifyTime,
};

/// A verified, newer bundle ready to [`Updater::fetch`].
#[derive(Debug, Clone)]
pub struct PendingUpdate {
    signed: SignedIndex,
    skipped: Vec<SkippedRecipe>,
}

impl PendingUpdate {
    /// The verified index.
    #[must_use]
    pub fn signed(&self) -> &SignedIndex {
        &self.signed
    }

    /// Offered version.
    #[must_use]
    pub fn version(&self) -> BundleVersion {
        self.signed.version()
    }

    /// Recipes this engine will install.
    #[must_use]
    pub fn recipes(&self) -> Vec<&RecipeEntry> {
        self.signed.index().partition().0
    }

    /// Recipes left out for an unsupported schema.
    #[must_use]
    pub fn skipped(&self) -> &[SkippedRecipe] {
        &self.skipped
    }
}

/// Result of [`Updater::check`].
#[derive(Debug, Clone)]
pub enum UpdateCheck {
    /// The offered bundle is the installed version.
    UpToDate(BundleVersion),
    /// A newer bundle is available.
    Available(PendingUpdate),
    /// The offered version was rolled back by the user; it is not offered
    /// again.
    Refused(BundleVersion),
}

/// What [`Updater::fetch`] transferred.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FetchStats {
    /// Blobs downloaded.
    pub fetched: usize,
    /// Blobs already in the store.
    pub reused: usize,
    /// Bytes downloaded.
    pub bytes: u64,
}

/// Result of [`Updater::update`].
#[derive(Debug, Clone)]
pub enum UpdateOutcome {
    /// Nothing newer was offered.
    UpToDate(BundleVersion),
    /// The offered version was rolled back by the user; nothing changed.
    Refused(BundleVersion),
    /// A newer bundle was fetched and installed.
    Installed {
        /// Install result.
        outcome: InstallOutcome,
        /// Transfer counts.
        stats: FetchStats,
        /// Installed recipes left out (unsupported schema or failing
        /// validation).
        skipped: Vec<SkippedRecipe>,
    },
}

/// Checks a bundle source for updates and fetches them.
#[derive(Debug)]
pub struct Updater<F> {
    fetcher: F,
    roots: TrustRoots,
}

impl<F: Fetcher> Updater<F> {
    /// Updater reading from `fetcher`, trusting `roots`.
    pub fn new(fetcher: F, roots: TrustRoots) -> Self {
        Self { fetcher, roots }
    }

    /// Fetch and verify the offered index at time `now` (unix seconds)
    /// and judge it against `store` by the update policy
    /// ([`crate::MAX_VERSION_JUMP`] and friends): an older or too-far-ahead
    /// version fails, a rolled-back one is [`UpdateCheck::Refused`]. An
    /// unsupported format fails.
    pub fn check(&self, store: &BundleStore<'_>, now: u64) -> Result<UpdateCheck> {
        self.check_against(&store.baseline()?, now)
    }

    fn check_against(&self, baseline: &Baseline, now: u64) -> Result<UpdateCheck> {
        let signed = fetch_signed(&self.fetcher, &self.roots, VerifyTime::Now(now))?;
        if let Some(pointer) = self.fetcher.release()
            && pointer != signed.version()
        {
            return Err(BundleError::PointerMismatch {
                pointer,
                index: signed.version(),
            });
        }
        Ok(match baseline.admit(signed.index())? {
            Admission::Current => UpdateCheck::UpToDate(signed.version()),
            Admission::Refused => {
                tracing::info!(version = %signed.version(), "offered recipe bundle was rolled back; not offered");
                UpdateCheck::Refused(signed.version())
            }
            Admission::Newer => {
                let skipped = signed.index().partition().1;
                if !skipped.is_empty() {
                    tracing::info!(count = skipped.len(), "recipes skipped: unsupported schema");
                }
                UpdateCheck::Available(PendingUpdate { signed, skipped })
            }
        })
    }

    /// Download into `store` only the blobs it lacks, verifying each.
    pub fn fetch(&self, pending: &PendingUpdate, store: &BundleStore<'_>) -> Result<FetchStats> {
        let mut stats = FetchStats::default();
        for (asset, blob) in pending.signed.index().required_blobs() {
            if store.has_blob(&asset, &blob)? {
                stats.reused += 1;
                continue;
            }
            let bytes = fetch_blob(&self.fetcher, &asset, &blob)?;
            store.put_blob(&asset, &blob, &bytes)?;
            stats.fetched += 1;
            stats.bytes += blob.size;
        }
        tracing::info!(
            fetched = stats.fetched,
            reused = stats.reused,
            "bundle blobs fetched"
        );
        Ok(stats)
    }

    /// [`Self::check`], [`Self::fetch`] and [`BundleStore::install`] in
    /// one, holding the store lock throughout so concurrent updates (other
    /// processes or threads) serialize.
    pub fn update(&self, store: &BundleStore<'_>, now: u64) -> Result<UpdateOutcome> {
        let lock = store.lock()?;
        let pending = match self.check_against(&store.baseline()?, now)? {
            UpdateCheck::UpToDate(v) => return Ok(UpdateOutcome::UpToDate(v)),
            UpdateCheck::Refused(v) => return Ok(UpdateOutcome::Refused(v)),
            UpdateCheck::Available(p) => p,
        };
        let stats = self.fetch(&pending, store)?;
        let outcome = store.install_locked(&lock, &pending.signed)?;
        let skipped = store.catalog(pending.signed.index())?.skipped;
        Ok(UpdateOutcome::Installed {
            outcome,
            stats,
            skipped,
        })
    }
}
