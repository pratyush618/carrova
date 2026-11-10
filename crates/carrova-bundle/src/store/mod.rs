//! The local bundle store under the user-data root.
//!
//! ```text
//! <user_data>/carrova/recipes/
//!   blobs/<hash>.<ext>        content-addressed recipe + detect blobs
//!   indexes/<n>/index.json    verified index of version n (+ index.sig.json)
//!   state.json                { current, previous, refused }
//!   lock                      advisory lock held by every state change
//!   tmp/                      temp files and staging dirs (per-process names)
//! ```
//!
//! A rename cannot atomically replace a non-empty directory, so instead
//! of swapping a `current/` directory the store stages each index into its
//! own `indexes/<n>/` and switches by atomically replacing the small
//! `state.json`. A crash leaves the old or the new state, never a mix.
//! Indexes and blobs not used by `current` or `previous` are pruned after
//! every switch.
//!
//! Reading never writes: a missing store is an empty one. Its directories
//! are created by the first change.
//!
//! Every change (install, rollback, reset) holds the store lock
//! ([`carrova_platform::Platform::lock_exclusive`]), so concurrent CLI and
//! app processes, and threads of one process, never interleave.
//!
//! Stored indexes are trusted no further than their signatures: each load
//! re-verifies the chain against the current roots at signing time, so a
//! root removed from the trusted list revokes every bundle it signed.

use carrova_crypto::sign::TrustRoots;
use carrova_platform::{FileLock, Platform, SemanticRoot};

use crate::catalog::check_blob;
use crate::policy::{Baseline, Installed};
use crate::{
    Asset, BlobRef, BundleError, BundleIndex, BundleVersion, Catalog, DetectIndex, Result,
    SignedIndex, VerifyTime,
};

mod change;
mod fsio;
mod layout;
mod prune;

use fsio::Fs;
use layout::{Layout, StoreState};

/// The installed bundle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledBundle {
    /// Its index (signature re-verified on load).
    pub index: BundleIndex,
    /// The version kept for [`BundleStore::rollback`], if any.
    pub previous: Option<BundleVersion>,
}

impl InstalledBundle {
    /// Installed version.
    #[must_use]
    pub fn version(&self) -> BundleVersion {
        self.index.version
    }
}

/// Result of [`BundleStore::install`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallOutcome {
    /// The bundle is now current.
    Installed {
        /// New version.
        version: BundleVersion,
        /// Version kept for rollback.
        previous: Option<BundleVersion>,
    },
    /// That version was already current; nothing changed.
    AlreadyCurrent(BundleVersion),
}

/// Proof that the caller holds the store lock (released on drop).
#[derive(Debug)]
pub(crate) struct StoreLock {
    _guard: FileLock,
}

/// Content-addressed recipe store with atomic install and one-step
/// rollback. All file access goes through the [`Platform`].
#[derive(Debug)]
pub struct BundleStore<'p> {
    fs: Fs<'p>,
    layout: Layout,
    roots: TrustRoots,
}

impl<'p> BundleStore<'p> {
    /// The store at `<user_data>/carrova/recipes`. Opening and reading
    /// never write (recipes load before any plan is approved): a missing
    /// store is an empty one, and its directories are created only by the
    /// first change ([`Self::put_blob`], install, rollback, reset). Stored
    /// bundles count as installed only while their signature chains verify
    /// against `roots`.
    pub fn open(platform: &'p dyn Platform, roots: TrustRoots) -> Result<Self> {
        Ok(Self {
            fs: Fs(platform),
            layout: Layout::under(platform.resolve_root(SemanticRoot::UserData)?),
            roots,
        })
    }

    /// Store root directory.
    #[must_use]
    pub fn root(&self) -> &std::path::Path {
        &self.layout.root
    }

    /// The installed bundle, if any. Fails with
    /// [`BundleError::InstalledUntrusted`] when its chain no longer
    /// verifies against the trusted roots.
    pub fn installed(&self) -> Result<Option<InstalledBundle>> {
        let state = StoreState::read(self.fs, &self.layout)?;
        let Some(current) = state.current else {
            return Ok(None);
        };
        Ok(Some(InstalledBundle {
            index: self.read_signed(current)?.index().clone(),
            previous: state.previous,
        }))
    }

    /// Whether the store holds `asset` with exactly the bytes of `blob`
    /// (a damaged copy counts as missing).
    pub fn has_blob(&self, asset: &Asset, blob: &BlobRef) -> Result<bool> {
        let path = self.layout.blob(asset);
        if !self.fs.exists(&path)? {
            return Ok(false);
        }
        Ok(self
            .fs
            .read_capped(&path, blob.size)
            .is_ok_and(|bytes| blob.matches(&bytes)))
    }

    /// Store a blob after checking it against `blob`.
    pub fn put_blob(&self, asset: &Asset, blob: &BlobRef, bytes: &[u8]) -> Result<()> {
        check_blob(asset, blob, bytes)?;
        self.layout.create(self.fs)?;
        self.fs
            .write_atomic(&self.layout.tmp(), &self.layout.blob(asset), bytes)
    }

    /// Parse and validate every supported recipe of the installed bundle,
    /// re-verifying each blob's hash and size.
    pub fn recipes(&self) -> Result<Catalog> {
        let installed = self.installed()?.ok_or(BundleError::NotInstalled)?;
        self.catalog(&installed.index)
    }

    /// Parse the supported recipes of a stored `index` (blobs re-verified).
    pub(crate) fn catalog(&self, index: &BundleIndex) -> Result<Catalog> {
        Catalog::from_index(index, |entry| self.read_blob(&entry.asset(), &entry.blob()))
    }

    /// The installed bundle's detection index (hash re-verified).
    pub fn detect_index(&self) -> Result<DetectIndex> {
        let installed = self.installed()?.ok_or(BundleError::NotInstalled)?;
        let index = &installed.index;
        DetectIndex::parse(&self.read_blob(&index.detect_asset(), &index.detect)?)
    }

    /// Take the store lock (blocks while another thread or process holds
    /// it), creating the store first: only changes lock.
    pub(crate) fn lock(&self) -> Result<StoreLock> {
        self.layout.create(self.fs)?;
        Ok(StoreLock {
            _guard: self.fs.0.lock_exclusive(&self.layout.lock())?,
        })
    }

    /// What an offered bundle is judged against: the verified installed
    /// bundle (an untrusted or damaged one counts as none, so it can never
    /// block a legitimate update) and the refused versions.
    pub(crate) fn baseline(&self) -> Result<Baseline> {
        let state = StoreState::read(self.fs, &self.layout)?;
        Ok(self.baseline_of(&state))
    }

    fn baseline_of(&self, state: &StoreState) -> Baseline {
        let installed = state.current.and_then(|version| {
            self.read_signed(version)
                .inspect_err(|e| tracing::warn!(error = %e, "installed recipe bundle ignored"))
                .ok()
                .map(|signed| Installed::of(signed.index()))
        });
        Baseline {
            installed,
            refused: state.refused.clone(),
        }
    }

    /// Read a stored blob and verify it.
    fn read_blob(&self, asset: &Asset, blob: &BlobRef) -> Result<Vec<u8>> {
        let path = self.layout.blob(asset);
        if !self.fs.exists(&path)? {
            return Err(BundleError::MissingBlob {
                name: asset.file_name(),
            });
        }
        let bytes = self.fs.read_capped(&path, blob.size)?;
        check_blob(asset, blob, &bytes)?;
        Ok(bytes)
    }

    /// Raw bytes of `asset` (index or signature) of stored index `version`.
    fn read_index_file(&self, version: BundleVersion, asset: &Asset) -> Result<Vec<u8>> {
        let path = self.layout.index_file(version, asset);
        self.fs.read_capped(&path, asset.max_size())
    }

    /// Stored index `version`, its signature chain re-verified against the
    /// current roots at signing time (expiry since then is irrelevant;
    /// revocation by root removal is not).
    fn read_signed(&self, version: BundleVersion) -> Result<SignedIndex> {
        let signed = SignedIndex::verify(
            self.read_index_file(version, &Asset::Index)?,
            self.read_index_file(version, &Asset::Signature)?,
            &self.roots,
            VerifyTime::SignedAt,
        )
        .map_err(|e| match e {
            BundleError::Signature(source) => BundleError::InstalledUntrusted { version, source },
            other => other,
        })?;
        if signed.version() != version {
            return Err(BundleError::InvalidIndex(format!(
                "indexes/{} holds version {}",
                version.0,
                signed.version()
            )));
        }
        Ok(signed)
    }
}
