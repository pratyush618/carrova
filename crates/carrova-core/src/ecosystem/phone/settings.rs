//! The user's settings for a linked phone on this computer (Settings ›
//! Calls and Bluetooth, Phone notifications), in the state directory as
//! `phone-settings.json`, `600`, written atomically. Holds no phone data.

use carrova_platform::Platform;
use serde::{Deserialize, Serialize};

use crate::error::{CoreError, Corruption, EncodeError, Result};
use crate::store::{ensure_dir, read_file, write_atomic};

/// Settings file name in the state directory.
const SETTINGS_FILE: &str = "phone-settings.json";

/// When a phone's incoming call rings on this computer ("Ring on this
/// computer").
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum RingHere {
    /// Always.
    Always,
    /// While the user is using this computer.
    #[default]
    WhileActive,
    /// Never.
    Never,
}

/// How a linked phone shows on this computer. Missing fields read as the
/// defaults.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(default)]
pub struct PhoneSettings {
    /// When incoming calls ring here.
    pub ring_here: RingHere,
    /// Hold notification pop-ups while this computer is not in use.
    pub only_while_active: bool,
    /// Show notification content while this computer is locked. Off, a
    /// locked screen shows only that notifications arrived.
    pub show_when_locked: bool,
}

/// The saved settings; the defaults if none were saved.
pub(crate) fn load(platform: &dyn Platform) -> Result<PhoneSettings> {
    let path = platform.state_dir()?.join(SETTINGS_FILE);
    match read_file(platform, &path)? {
        Some(bytes) => serde_json::from_slice(&bytes).map_err(|e| CoreError::CorruptFile {
            file: SETTINGS_FILE.to_owned(),
            reason: Corruption::from(e),
        }),
        None => Ok(PhoneSettings::default()),
    }
}

pub(crate) fn save(platform: &dyn Platform, settings: &PhoneSettings) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(settings).map_err(|e| CoreError::Encode {
        what: "phone settings".to_owned(),
        source: EncodeError::from(e),
    })?;
    let dir = platform.state_dir()?;
    ensure_dir(platform, &dir, 0o700)?;
    write_atomic(platform, &dir, SETTINGS_FILE, &bytes)
}
