//! User settings.

use std::sync::Arc;

use carrova_core::devices::KnownDevices;
use carrova_view::ipc::{ErrorCode, Settings};
use serde_json::Value;

use crate::command_error::CommandError;
use crate::settings::patched;
use crate::state::AppState;
use crate::worker::blocking;

/// The saved settings (defaults if none).
///
/// # Errors
///
/// The settings file exists but cannot be read.
pub async fn load(state: &AppState) -> Result<Settings, CommandError> {
    let store = state.settings.clone();
    blocking(move || store.load()).await
}

/// Apply `patch` (the changed fields, see [`patched`]) to the saved
/// settings. Background settings change only through
/// [`set_background`](super::background::set_background), which keeps
/// start at sign-in in step with them. Every save with wake hints off
/// saves the setting, then drops every saved hint: a move remembered
/// meanwhile asks the saved setting under the devices lock, so none comes
/// back. Hints are dropped even if the save fails (it may have replaced
/// the file), and one an earlier save left behind goes with the next. If
/// the hints cannot be dropped while turning them off, the setting is put
/// back on; while already off, that is only logged.
///
/// # Errors
///
/// The patch changes background settings or does not fit, the settings
/// file cannot be read or written, or the hints cannot be dropped while
/// turning them off.
pub async fn save(state: &AppState, patch: Value) -> Result<(), CommandError> {
    if patch.get("background").is_some() {
        return Err(CommandError::new(
            ErrorCode::Internal,
            "background settings change through set_background",
        ));
    }
    let store = state.settings.clone();
    let platform = Arc::clone(&state.platform);
    blocking(move || {
        let _writing = store.lock();
        let old = store.load()?;
        let new = patched(&old, patch)?;
        let saved = store.save(&new);
        if new.nearby.wol_hints {
            return saved;
        }
        let platform = platform.as_ref();
        // No device store yet: nothing to clear, and none is made for it.
        let cleared = KnownDevices::open_existing(platform)
            .and_then(|k| k.map_or(Ok(()), |k| k.clear_wake_hints(platform)));
        // Logged first: a failed save or rollback below replaces it.
        if let Err(e) = &cleared {
            tracing::warn!(error = %e, "could not drop the wake hints");
        }
        saved?;
        if let Err(e) = cleared
            && old.nearby.wol_hints
        {
            store.save(&old)?;
            return Err(e.into());
        }
        Ok(())
    })
    .await
}
