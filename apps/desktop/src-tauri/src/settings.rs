//! The persisted user [`Settings`]: one JSON file under Carrova's state
//! directory, replaced atomically (temp file, fsync, rename, fsync dir).

use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use carrova_platform::{Platform, PlatformError};
use carrova_view::ipc::{NearbySettings, SETTINGS_FILE, Settings};
use serde_json::Value;

use crate::command_error::CommandError;

/// Reads and writes [`Settings`].
#[derive(Debug, Clone)]
pub struct SettingsStore {
    platform: Arc<dyn Platform>,
    /// Held across each read-change-save; clones share it.
    writing: Arc<Mutex<()>>,
}

impl SettingsStore {
    /// The store of `platform`'s state directory.
    pub fn new(platform: Arc<dyn Platform>) -> Self {
        Self {
            platform,
            writing: Arc::default(),
        }
    }

    /// Hold while reading, changing and saving the settings: writers then
    /// never drop each other's changes, nor share the temp file.
    pub fn lock(&self) -> MutexGuard<'_, ()> {
        self.writing.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn path(&self) -> Result<PathBuf, PlatformError> {
        Ok(self.platform.state_dir()?.join(SETTINGS_FILE))
    }

    /// The saved settings; the defaults if none were saved yet.
    ///
    /// # Errors
    ///
    /// The file exists but cannot be read or parsed.
    pub fn load(&self) -> Result<Settings, CommandError> {
        Ok(Settings::load(self.platform.as_ref())?)
    }

    /// Whether new engines remember devices (`nearby.remember`).
    pub fn remember_devices(&self) -> bool {
        self.nearby().remember
    }

    /// Whether new engines save wake hints (`nearby.wol_hints`).
    pub fn wol_hints(&self) -> bool {
        self.nearby().wol_hints
    }

    fn nearby(&self) -> NearbySettings {
        NearbySettings::saved(self.platform.as_ref())
    }

    /// Replace the saved settings with `settings`.
    ///
    /// # Errors
    ///
    /// The state directory or the file cannot be written.
    pub fn save(&self, settings: &Settings) -> Result<(), CommandError> {
        let path = self.path()?;
        let dir = self.platform.state_dir()?;
        std::fs::create_dir_all(&dir)?;
        let tmp = dir.join(format!("{SETTINGS_FILE}.tmp"));
        // A temp file left by a crash is stale: start over.
        if let Err(e) = self.platform.remove_file(&tmp) {
            tracing::debug!(error = %e, "no stale settings temp file removed");
        }
        let mut file = self.platform.create_new_file(&tmp, Some(0o600))?;
        file.write_all(&serde_json::to_vec_pretty(settings)?)?;
        file.sync_all()?;
        drop(file);
        self.platform.rename_replace(&tmp, &path)?;
        self.platform.sync_dir(&dir)?;
        Ok(())
    }
}

/// `settings` with `patch` applied as a JSON merge patch (RFC 7396):
/// objects merge field by field, `null` clears a field and whatever the
/// patch leaves out stays, so a change made elsewhere meanwhile (the tray's
/// pause) survives.
///
/// # Errors
///
/// The patched settings are not valid settings.
pub fn patched(settings: &Settings, patch: Value) -> Result<Settings, CommandError> {
    let mut value = serde_json::to_value(settings)?;
    merge(&mut value, patch);
    Ok(serde_json::from_value(value)?)
}

fn merge(target: &mut Value, patch: Value) {
    let Value::Object(fields) = patch else {
        *target = patch;
        return;
    };
    if !target.is_object() {
        *target = Value::Object(serde_json::Map::new());
    }
    if let Value::Object(target) = target {
        for (key, value) in fields {
            if value.is_null() {
                target.remove(&key);
            } else {
                merge(target.entry(key).or_insert(Value::Null), value);
            }
        }
    }
}

#[cfg(test)]
#[path = "../tests/settings/unit.rs"]
mod tests;
