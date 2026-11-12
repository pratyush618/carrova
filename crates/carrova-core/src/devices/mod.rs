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

use std::path::PathBuf;

use carrova_crypto::identity::{DEVICE_SEED_LEN, DeviceId, DeviceKey, Fingerprint};
use carrova_crypto::pairing::{KNOWN_SECRET_LEN, KnownSecret};
use carrova_platform::{FileKind, FileLock, Platform};
use serde::{Deserialize, Serialize};

pub use record::DeviceRecord;

use crate::error::{CoreError, Corruption, EncodeError, Result};
use crate::fsx;
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

/// `known.json`. Unknown fields are ignored (and dropped on the next
/// write), never a reason to reset.
#[derive(Serialize, Deserialize)]
struct KnownFile {
    version: u16,
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

    /// This device's identity key, made on first need. Two processes
    /// racing to make it agree on the first one written.
    pub fn device_key(&self, platform: &dyn Platform) -> Result<DeviceKey> {
        if let Some(key) = self.read_device_key(platform)? {
            return Ok(key);
        }
        let key = DeviceKey::generate().map_err(|_| CoreError::Random)?;
        match secret_file::write_new(
            platform,
            &self.state_dir,
            DEVICE_KEY_FILE,
            key.expose_seed(),
        ) {
            Ok(()) => Ok(key),
            Err(e) if fsx::kind_of(&e) == Some(std::io::ErrorKind::AlreadyExists) => {
                self.read_device_key(platform)?.ok_or(e)
            }
            Err(e) => Err(e),
        }
    }

    /// Every known device, in the order first remembered.
    pub fn list(&self, platform: &dyn Platform) -> Result<Vec<DeviceRecord>> {
        Ok(self.load(platform)?.devices)
    }

    /// Remember `record`'s device after a code pairing, with the `secret`
    /// both sides now share. Remembering a known device again replaces its
    /// secret (latest pairing wins) and its OS and MAC hint, but keeps the
    /// local name, first-seen time and move history. The record is saved
    /// before the secret, so a crash never leaves a secret without one.
    pub fn remember(
        &self,
        platform: &dyn Platform,
        record: DeviceRecord,
        secret: &KnownSecret,
    ) -> Result<()> {
        let _lock = self.lock(platform)?;
        let mut file = self.load(platform)?;
        let name = key_file_name(&record.public);
        match file.devices.iter_mut().find(|d| d.public == record.public) {
            Some(known) => {
                known.os = record.os;
                known.mac_hint = record.mac_hint;
            }
            None => file.devices.push(record),
        }
        self.save(platform, &file)?;
        secret_file::replace(platform, &self.dir, &name, secret.expose_bytes())
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

    /// Give a known device a new local name.
    pub fn rename(&self, platform: &dyn Platform, id: &DeviceId, name: String) -> Result<()> {
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
        for entry in platform.read_dir(&self.dir)? {
            let name = entry.name.to_string_lossy();
            // `<id>.key` and a stale `<id>.key.new` from a crash.
            if entry.kind != FileKind::Dir && name.contains(KEY_SUFFIX) {
                secret_file::wipe(platform, &self.dir, &name, KNOWN_SECRET_LEN)?;
            }
        }
        self.save(
            platform,
            &KnownFile {
                version: KNOWN_VERSION,
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
