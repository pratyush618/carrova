//! Known devices (ADR 0001): this install's identity key and the devices
//! it has paired with and remembers.
//!
//! Files under `Platform::state_dir()`, dirs `700`, files `600`:
//!
//! | File | Content |
//! |---|---|
//! | `device.key` | this device's 32-byte Ed25519 seed |
//! | `devices/known.json` | versioned list of [`DeviceRecord`]s |
//! | `devices/<public key hex>.key` | the 32-byte `KnownSecret` shared with that device |
//!
//! Everything is keyed by the full public key ([`DeviceId`]); fingerprints
//! are display labels ([`KnownDevices::resolve`] turns a typed one into
//! exactly one device). Key files are written, checked and wiped like a
//! session's `resume.key`. A bad `device.key` or `known.json` is
//! reported, never regenerated or overwritten, except by the explicit
//! [`KnownDevices::forget_all`] and [`KnownDevices::reset_identity`].
//! Nothing here logs keys, secrets, full public keys or MAC addresses.
//!
//! Every change runs under an exclusive lock on `devices/known.lock`, so
//! the background app and the CLI never lose each other's updates; reads
//! need none (`known.json` is replaced atomically).

mod record;
mod wake;

use std::path::PathBuf;

use carrova_crypto::identity::{DEVICE_SEED_LEN, DeviceId, DeviceKey, Fingerprint};
use carrova_crypto::pairing::{KNOWN_SECRET_LEN, KnownSecret};
use carrova_platform::{FileKind, FileLock, Platform};
use serde::{Deserialize, Serialize};

pub use record::{DeviceRecord, HintUpdate};

use crate::error::{CoreError, Corruption, EncodeError, Result};
use crate::fsx;
use crate::lan::clean_device_name;
use crate::store::{ensure_dir, read_file, secret_file, write_atomic};

/// This device's identity key, in the state dir.
pub const DEVICE_KEY_FILE: &str = "device.key";
/// Directory of the known-device store, in the state dir.
pub const DEVICES_DIR: &str = "devices";
/// The record list, in [`DEVICES_DIR`].
pub const KNOWN_FILE: &str = "known.json";
/// Lock serialising changes, in [`DEVICES_DIR`].
const LOCK_FILE: &str = "known.lock";
/// Version written to [`KNOWN_FILE`]; newer files are refused.
const KNOWN_VERSION: u16 = 1;
/// Suffix of a known device's key file.
const KEY_SUFFIX: &str = ".key";
/// Suffix of a key file being replaced (see `secret_file::replace`).
const NEW_KEY_SUFFIX: &str = ".key.new";

/// `known.json`. Unknown fields are ignored (and dropped on the next
/// write), never a reason to reset.
#[derive(Serialize, Deserialize)]
struct KnownFile {
    version: u16,
    /// Bumped by every forget (see [`KnownDevices::epoch`]); older files
    /// read as 0.
    #[serde(default)]
    epoch: u64,
    devices: Vec<DeviceRecord>,
}

/// Handle to the known-device store.
#[derive(Debug, Clone)]
pub struct KnownDevices {
    state_dir: PathBuf,
    dir: PathBuf,
}

impl KnownDevices {
    /// Open (create if needed, mode 700) `state_dir()/devices`.
    pub fn open(platform: &dyn Platform) -> Result<Self> {
        let state_dir = platform.state_dir()?;
        let dir = state_dir.join(DEVICES_DIR);
        ensure_dir(platform, &dir, 0o700)?;
        Ok(Self { state_dir, dir })
    }

    /// The store if it exists; `None` without creating it (read-only
    /// callers: browsing, the presence advert).
    pub fn open_existing(platform: &dyn Platform) -> Result<Option<Self>> {
        let dir = platform.state_dir()?.join(DEVICES_DIR);
        if fsx::lstat(platform, &dir)?.is_none() {
            return Ok(None);
        }
        Self::open(platform).map(Some)
    }

    /// This device's identity key, made on first need (under the lock,
    /// written as `device.key.new` and renamed into place, so a crash never
    /// leaves a partial key and racing processes agree on one key).
    pub fn device_key(&self, platform: &dyn Platform) -> Result<DeviceKey> {
        if let Some(key) = self.read_device_key(platform)? {
            return Ok(key);
        }
        let _lock = self.lock(platform)?;
        // Another process may have made it while we waited.
        if let Some(key) = self.read_device_key(platform)? {
            return Ok(key);
        }
        self.new_device_key(platform)
    }

    /// Every known device, in the order first remembered.
    pub fn list(&self, platform: &dyn Platform) -> Result<Vec<DeviceRecord>> {
        Ok(self.load(platform)?.devices)
    }

    /// Remember `record`'s device after a code pairing, with the `secret`
    /// both sides now share. Remembering a known device again replaces its
    /// secret (latest pairing wins), its OS and any new MAC hint (an empty
    /// hint keeps the old one), but keeps the local name, first-seen time
    /// and move history. The record is saved before the secret, so a crash
    /// never leaves a secret without one.
    pub fn remember(
        &self,
        platform: &dyn Platform,
        record: DeviceRecord,
        secret: &KnownSecret,
    ) -> Result<()> {
        let hint = record.mac_hint.map_or(HintUpdate::Keep, HintUpdate::Set);
        self.remember_locked(platform, record, secret, || hint, None)
            .map(drop)
    }

    /// [`Self::remember`] after a finished move at `at` (unix seconds),
    /// counted in the same locked step, with the device's wake hint
    /// updated by what `hint` says under the lock (`record.mac_hint` is
    /// only the starting hint of a new device); returns the updated record.
    /// `None`, storing nothing, if devices were forgotten since the move
    /// read `epoch` ([`Self::epoch`]) at pairing.
    pub fn remember_move(
        &self,
        platform: &dyn Platform,
        record: DeviceRecord,
        secret: &KnownSecret,
        hint: impl FnOnce() -> HintUpdate,
        at: u64,
        epoch: u64,
    ) -> Result<Option<DeviceRecord>> {
        self.remember_locked(platform, record, secret, hint, Some((at, epoch)))
    }

    /// Changes each time a device is forgotten ([`Self::forget`],
    /// [`Self::forget_all`], [`Self::reset_identity`]): a move reads it
    /// when it pairs, so its remember cannot undo a forget made meanwhile.
    pub fn epoch(&self, platform: &dyn Platform) -> Result<u64> {
        Ok(self.load(platform)?.epoch)
    }

    /// Drop every saved wake hint (the user turned hints off); writes
    /// nothing when there is none.
    pub fn clear_wake_hints(&self, platform: &dyn Platform) -> Result<()> {
        let _lock = self.lock(platform)?;
        let mut file = self.load(platform)?;
        if file.devices.iter().all(|d| d.mac_hint.is_none()) {
            return Ok(());
        }
        for device in &mut file.devices {
            device.mac_hint = None;
        }
        self.save(platform, &file)
    }

    /// Remember `record` with `secret` under the lock, applying `hint` and
    /// counting a move finished at `moved` (unix seconds, the epoch read at
    /// its pairing) if any; `None` if that epoch is stale.
    fn remember_locked(
        &self,
        platform: &dyn Platform,
        record: DeviceRecord,
        secret: &KnownSecret,
        hint: impl FnOnce() -> HintUpdate,
        moved: Option<(u64, u64)>,
    ) -> Result<Option<DeviceRecord>> {
        let _lock = self.lock(platform)?;
        let mut file = self.load(platform)?;
        if moved.is_some_and(|(_, epoch)| epoch != file.epoch) {
            return Ok(None);
        }
        let name = key_file_name(&record.public);
        let index = match file.devices.iter().position(|d| d.public == record.public) {
            Some(i) => {
                let known = &mut file.devices[i];
                known.os = record.os;
                i
            }
            None => {
                file.devices.push(record);
                file.devices.len() - 1
            }
        };
        let device = &mut file.devices[index];
        device.mac_hint = hint().apply(device.mac_hint);
        if let Some((at, _)) = moved {
            device.moves = device.moves.saturating_add(1);
            device.last_move = Some(at);
        }
        let updated = device.clone();
        self.save(platform, &file)?;
        secret_file::replace(platform, &self.dir, &name, secret.expose_bytes())?;
        Ok(Some(updated))
    }

    /// The secret shared with a known device; `None` if the device is not
    /// known or its key file is gone (a record without a secret is
    /// harmless: the device is simply not recognised).
    pub fn secret_for(
        &self,
        platform: &dyn Platform,
        id: &DeviceId,
    ) -> Result<Option<KnownSecret>> {
        if !self.load(platform)?.devices.iter().any(|d| d.public == *id) {
            return Ok(None);
        }
        let bytes = secret_file::read::<KNOWN_SECRET_LEN>(
            platform,
            &self.dir.join(key_file_name(id)),
            |problem| CoreError::DeviceKey {
                key: format!("the key of {}", Fingerprint::of(id)),
                problem,
            },
        )?;
        if bytes.is_none() {
            tracing::debug!(device = %Fingerprint::of(id), "known device has no key file");
        }
        Ok(bytes.map(|b| KnownSecret::from_bytes(*b)))
    }

    /// Give a known device a new local name, cleaned like a peer's
    /// announced name (control and bidi characters dropped, trimmed, at
    /// most [`MAX_DEVICE_NAME`](crate::lan::MAX_DEVICE_NAME) characters).
    /// [`CoreError::InvalidDeviceName`] if nothing is left.
    pub fn rename(&self, platform: &dyn Platform, id: &DeviceId, name: &str) -> Result<()> {
        let name = clean_device_name(name).ok_or(CoreError::InvalidDeviceName)?;
        let _lock = self.lock(platform)?;
        let mut file = self.load(platform)?;
        let device = file
            .devices
            .iter_mut()
            .find(|d| d.public == *id)
            .ok_or(CoreError::UnknownDevice)?;
        device.name = name;
        self.save(platform, &file)
    }

    /// Forget one device: its key file is wiped first, then its record
    /// removed. `false` if it was not known.
    pub fn forget(&self, platform: &dyn Platform, id: &DeviceId) -> Result<bool> {
        let _lock = self.lock(platform)?;
        let mut file = self.load(platform)?;
        secret_file::wipe(platform, &self.dir, &key_file_name(id), KNOWN_SECRET_LEN)?;
        let before = file.devices.len();
        file.devices.retain(|d| d.public != *id);
        if file.devices.len() == before {
            return Ok(false);
        }
        file.epoch = file.epoch.wrapping_add(1);
        self.save(platform, &file)?;
        Ok(true)
    }

    /// Forget every device: every key file in the store is wiped, then the
    /// record list emptied (even if it was corrupt: this is the user's
    /// explicit way out).
    pub fn forget_all(&self, platform: &dyn Platform) -> Result<()> {
        let _lock = self.lock(platform)?;
        self.forget_all_locked(platform)
    }

    /// [`Self::forget_all`] under a held lock.
    fn forget_all_locked(&self, platform: &dyn Platform) -> Result<()> {
        // A corrupt list's epoch is unknown: count on from 0.
        let epoch = self.load(platform).map_or(0, |f| f.epoch).wrapping_add(1);
        for entry in platform.read_dir(&self.dir)? {
            let name = entry.name.to_string_lossy();
            // `<id>.key` and a stale `<id>.key.new` from a crash.
            let key_file = name.ends_with(KEY_SUFFIX) || name.ends_with(NEW_KEY_SUFFIX);
            if entry.kind != FileKind::Dir && key_file {
                secret_file::wipe(platform, &self.dir, &name, KNOWN_SECRET_LEN)?;
            }
        }
        self.save(
            platform,
            &KnownFile {
                version: KNOWN_VERSION,
                epoch,
                devices: Vec::new(),
            },
        )
    }

    /// Reset this computer's identity: forget every device, then replace
    /// the device key with a new one (returned). Known peers no longer
    /// recognise this device until they pair with it again.
    pub fn reset_identity(&self, platform: &dyn Platform) -> Result<DeviceKey> {
        let _lock = self.lock(platform)?;
        self.forget_all_locked(platform)?;
        self.new_device_key(platform)
    }

    /// Make and store a new device key (caller holds the lock).
    fn new_device_key(&self, platform: &dyn Platform) -> Result<DeviceKey> {
        let key = DeviceKey::generate().map_err(|_| CoreError::Random)?;
        secret_file::replace(
            platform,
            &self.state_dir,
            DEVICE_KEY_FILE,
            key.expose_seed(),
        )?;
        Ok(key)
    }

    /// The one known device `query` names: a fingerprint (`7F3A·19C2`,
    /// any case, separator optional) or the full public key in hex.
    /// [`CoreError::UnknownDevice`] if none matches,
    /// [`CoreError::AmbiguousDevice`] if several do.
    pub fn resolve(&self, platform: &dyn Platform, query: &str) -> Result<DeviceRecord> {
        let devices = self.list(platform)?;
        let mut matches: Vec<DeviceRecord> = if let Ok(id) = query.parse::<DeviceId>() {
            devices.into_iter().filter(|d| d.public == id).collect()
        } else if let Ok(fp) = query.parse::<Fingerprint>() {
            devices
                .into_iter()
                .filter(|d| d.fingerprint() == fp)
                .collect()
        } else {
            Vec::new()
        };
        match matches.len() {
            0 => Err(CoreError::UnknownDevice),
            1 => Ok(matches.remove(0)),
            n => Err(CoreError::AmbiguousDevice { matches: n }),
        }
    }

    /// The id of the one known device `query` names ([`Self::resolve`]):
    /// what commands taking a typed device act on.
    pub fn resolve_id(&self, platform: &dyn Platform, query: &str) -> Result<DeviceId> {
        Ok(self.resolve(platform, query)?.public)
    }

    /// Exclusive lock for one change (released on drop).
    fn lock(&self, platform: &dyn Platform) -> Result<FileLock> {
        Ok(platform.lock_exclusive(&self.dir.join(LOCK_FILE))?)
    }

    fn read_device_key(&self, platform: &dyn Platform) -> Result<Option<DeviceKey>> {
        let seed = secret_file::read::<DEVICE_SEED_LEN>(
            platform,
            &self.state_dir.join(DEVICE_KEY_FILE),
            |problem| CoreError::DeviceKey {
                key: DEVICE_KEY_FILE.into(),
                problem,
            },
        )?;
        Ok(seed.map(|s| DeviceKey::from_bytes(&s)))
    }

    /// The record list; empty if there is none yet.
    fn load(&self, platform: &dyn Platform) -> Result<KnownFile> {
        let Some(bytes) = read_file(platform, &self.dir.join(KNOWN_FILE))? else {
            return Ok(KnownFile {
                version: KNOWN_VERSION,
                epoch: 0,
                devices: Vec::new(),
            });
        };
        let corrupt = |reason| CoreError::CorruptFile {
            file: format!("{DEVICES_DIR}/{KNOWN_FILE}"),
            reason,
        };
        let file: KnownFile =
            serde_json::from_slice(&bytes).map_err(|e| corrupt(Corruption::from(e)))?;
        if file.version > KNOWN_VERSION {
            return Err(corrupt(Corruption::Version(file.version)));
        }
        Ok(file)
    }

    fn save(&self, platform: &dyn Platform, file: &KnownFile) -> Result<()> {
        let bytes = serde_json::to_vec_pretty(file).map_err(|e| CoreError::Encode {
            what: KNOWN_FILE.to_owned(),
            source: EncodeError::from(e),
        })?;
        write_atomic(platform, &self.dir, KNOWN_FILE, &bytes)
    }
}

/// `<public key hex>.key`.
fn key_file_name(id: &DeviceId) -> String {
    format!("{}{KEY_SUFFIX}", id.to_hex())
}
