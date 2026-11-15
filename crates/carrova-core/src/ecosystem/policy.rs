//! Per-device feature policies, one file per known device.
//!
//! `devices/policy/<public key hex>.json`, dir `700`, files `600`, written
//! atomically. A device with no file reads as
//! [`PolicyDoc::computer_defaults`] (everything off), so computers known
//! before linked devices existed gain no feature. Phones get their
//! defaults written at pairing. Forgetting a device (or resetting the
//! identity) deletes its policy with its key.
//!
//! shortcut: `set` is read-modify-write without a lock, so two processes
//! changing one device's policy at once can lose an update; add the
//! `known.lock` if the background app and CLI start writing concurrently.

use std::path::PathBuf;

use carrova_crypto::identity::DeviceId;
use carrova_link::{Feature, FeatureState, PolicyDoc};
use carrova_platform::{FileKind, Platform};

use crate::devices::DEVICES_DIR;
use crate::error::{CoreError, Corruption, EncodeError, Result};
use crate::fsx;
use crate::store::{ensure_dir, read_file, write_atomic};

/// Directory of the policy files, in [`DEVICES_DIR`].
pub(crate) const POLICY_DIR: &str = "policy";
/// Version written to policy files; newer files are refused.
const POLICY_VERSION: u32 = 1;

/// Handle to the per-device policy files.
#[derive(Debug, Clone)]
pub struct PolicyStore {
    dir: PathBuf,
}

impl PolicyStore {
    /// Open (create if needed, mode 700) `state_dir()/devices/policy`.
    pub fn open(platform: &dyn Platform) -> Result<Self> {
        let devices = platform.state_dir()?.join(DEVICES_DIR);
        ensure_dir(platform, &devices, 0o700)?;
        let dir = devices.join(POLICY_DIR);
        ensure_dir(platform, &dir, 0o700)?;
        Ok(Self { dir })
    }

    /// The store over an existing `devices/policy` directory, for the
    /// known-device store's own cleanup (which does not create it).
    pub(crate) fn at(dir: PathBuf) -> Self {
        Self { dir }
    }

    /// `id`'s policy; its computer defaults (all off) if none is stored.
    pub fn get(&self, platform: &dyn Platform, id: &DeviceId) -> Result<PolicyDoc> {
        let name = file_name(id);
        let Some(bytes) = read_file(platform, &self.dir.join(&name))? else {
            return Ok(PolicyDoc::computer_defaults());
        };
        let corrupt = |reason| CoreError::CorruptFile {
            file: format!("{DEVICES_DIR}/{POLICY_DIR}/<device>.json"),
            reason,
        };
        let doc: PolicyDoc =
            serde_json::from_slice(&bytes).map_err(|e| corrupt(Corruption::from(e)))?;
        if doc.version > POLICY_VERSION {
            let version = u16::try_from(doc.version).unwrap_or(u16::MAX);
            return Err(corrupt(Corruption::Version(version)));
        }
        Ok(doc)
    }

    /// Set one feature of `id`'s policy and return the result. The state
    /// must fit the feature ([`CoreError::Link`] otherwise, nothing
    /// written).
    pub fn set(
        &self,
        platform: &dyn Platform,
        id: &DeviceId,
        feature: Feature,
        state: FeatureState,
    ) -> Result<PolicyDoc> {
        let mut doc = self.get(platform, id)?;
        doc.features.insert(feature, state);
        let doc = doc.validated()?;
        self.put(platform, id, &doc)?;
        Ok(doc)
    }

    /// Store `doc` as `id`'s policy.
    pub fn put(&self, platform: &dyn Platform, id: &DeviceId, doc: &PolicyDoc) -> Result<()> {
        let name = file_name(id);
        let bytes = serde_json::to_vec_pretty(doc).map_err(|e| CoreError::Encode {
            what: "device policy".to_owned(),
            source: EncodeError::from(e),
        })?;
        write_atomic(platform, &self.dir, &name, &bytes)
    }

    /// Store `doc` as `id`'s policy unless it already has a file (a
    /// re-pairing keeps what the user chose).
    pub(crate) fn put_if_absent(
        &self,
        platform: &dyn Platform,
        id: &DeviceId,
        doc: &PolicyDoc,
    ) -> Result<()> {
        if fsx::lstat(platform, &self.dir.join(file_name(id)))?.is_some() {
            return Ok(());
        }
        self.put(platform, id, doc)
    }

    /// Delete `id`'s policy; nothing if there is none.
    pub fn remove(&self, platform: &dyn Platform, id: &DeviceId) -> Result<()> {
        let path = self.dir.join(file_name(id));
        if fsx::lstat(platform, &path)?.is_some() {
            platform.remove_file(&path)?;
        }
        Ok(())
    }

    /// Delete every policy file (and stale temp files); nothing if the
    /// directory does not exist.
    pub(crate) fn remove_all(&self, platform: &dyn Platform) -> Result<()> {
        if fsx::lstat(platform, &self.dir)?.is_none() {
            return Ok(());
        }
        for entry in platform.read_dir(&self.dir)? {
            if entry.kind != FileKind::Dir {
                platform.remove_file(&self.dir.join(&entry.name))?;
            }
        }
        Ok(())
    }
}

/// `<public key hex>.json`.
fn file_name(id: &DeviceId) -> String {
    format!("{}.json", id.to_hex())
}
