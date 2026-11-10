//! The persisted user [`Settings`]: one JSON file under Carrova's state
//! directory, replaced atomically (temp file, fsync, rename, fsync dir).

use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;

use carrova_platform::{Platform, PlatformError};
use carrova_view::ipc::Settings;

use crate::command_error::CommandError;

/// Settings file name in the state directory.
pub const SETTINGS_FILE: &str = "desktop-settings.json";

/// Reads and writes [`Settings`].
#[derive(Debug, Clone)]
pub struct SettingsStore {
    platform: Arc<dyn Platform>,
}

impl SettingsStore {
    /// The store of `platform`'s state directory.
    pub fn new(platform: Arc<dyn Platform>) -> Self {
        Self { platform }
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
        let path = self.path()?;
        match std::fs::read(&path) {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Settings::default()),
            Err(e) => Err(e.into()),
        }
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

#[cfg(test)]
#[path = "../tests/settings/unit.rs"]
mod tests;
