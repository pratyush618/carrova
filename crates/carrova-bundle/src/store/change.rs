//! Changing the installed bundle: install, rollback, reset. Each holds
//! the store lock for its whole read-modify-write.

use super::{BundleStore, InstallOutcome, StoreLock, fsio, layout::StoreState};
use crate::policy::Admission;
use crate::{Asset, BundleError, BundleVersion, Result, SignedIndex};

impl BundleStore<'_> {
    /// Make `signed` the current bundle. Every blob it needs must already
    /// be stored ([`crate::Updater::fetch`]). The update policy
    /// ([`crate::MAX_VERSION_JUMP`] and friends) applies: an older,
    /// too-far-ahead or rolled-back version is refused; the same version
    /// is a no-op.
    pub fn install(&self, signed: &SignedIndex) -> Result<InstallOutcome> {
        let lock = self.lock()?;
        self.install_locked(&lock, signed)
    }

    /// [`Self::install`] for a caller already holding the lock.
    pub(crate) fn install_locked(
        &self,
        _lock: &StoreLock,
        signed: &SignedIndex,
    ) -> Result<InstallOutcome> {
        let version = signed.version();
        let state = StoreState::read(self.fs, &self.layout)?;
        let baseline = self.baseline_of(&state);
        match baseline.admit(signed.index())? {
            Admission::Current => return Ok(InstallOutcome::AlreadyCurrent(version)),
            Admission::Refused => return Err(BundleError::Refused(version)),
            Admission::Newer => {}
        }
        for (asset, blob) in signed.index().required_blobs() {
            self.read_blob(&asset, &blob)?;
        }
        self.write_index(signed)?;
        let previous = baseline.installed.map(|i| i.version);
        StoreState {
            current: Some(version),
            previous,
            refused: state.refused,
        }
        .write(self.fs, &self.layout)?;
        tracing::info!(%version, ?previous, "recipe bundle installed");
        self.prune();
        Ok(InstallOutcome::Installed { version, previous })
    }

    /// Make the previous bundle current again and drop the newer one,
    /// which is remembered as refused: it is never offered again.
    pub fn rollback(&self) -> Result<BundleVersion> {
        let _lock = self.lock()?;
        let mut state = StoreState::read(self.fs, &self.layout)?;
        let current = state.current.ok_or(BundleError::NotInstalled)?;
        let previous = state.previous.ok_or(BundleError::NoPrevious)?;
        self.read_signed(previous)?;
        state.refused.insert(current);
        StoreState {
            current: Some(previous),
            previous: None,
            refused: state.refused,
        }
        .write(self.fs, &self.layout)?;
        tracing::info!(from = %current, to = %previous, "recipe bundle rolled back");
        self.prune();
        Ok(previous)
    }

    /// Remove the installed bundle (and the one kept for rollback), so the
    /// bundle shipped with the app is used again. Rolled-back versions
    /// stay refused. Returns the version that was installed, if any.
    pub fn reset(&self) -> Result<Option<BundleVersion>> {
        let _lock = self.lock()?;
        let state = StoreState::read(self.fs, &self.layout)?;
        let removed = state.current;
        StoreState {
            current: None,
            previous: None,
            refused: state.refused,
        }
        .write(self.fs, &self.layout)?;
        tracing::info!(?removed, "recipe bundle store reset");
        self.prune();
        Ok(removed)
    }

    /// Stage `indexes/<n>/` in `tmp/` and move it into place.
    fn write_index(&self, signed: &SignedIndex) -> Result<()> {
        let dest = self.layout.index_dir(signed.version());
        let staging = fsio::temp_path(&self.layout.tmp(), &dest);
        if self.fs.exists(&staging)? {
            self.fs.remove_tree(&staging, &self.layout.root)?;
        }
        self.fs.ensure_dir(&staging)?;
        for (asset, bytes) in [
            (Asset::Index, signed.index_bytes()),
            (Asset::Signature, signed.sig_bytes()),
        ] {
            self.fs.write_new(&staging.join(asset.file_name()), bytes)?;
        }
        self.fs.sync_parent(&staging.join(Asset::INDEX_FILE))?;
        if self.fs.exists(&dest)? {
            // Left by an earlier install of the same version that was
            // rolled back past; never live (the state says otherwise).
            self.fs.remove_tree(&dest, &self.layout.root)?;
        }
        self.fs.rename_noreplace(&staging, &dest)?;
        self.fs.sync_parent(&dest)
    }
}
