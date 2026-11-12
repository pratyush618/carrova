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
/// start at sign-in in step with them. Turning wake hints off first drops
/// every saved hint (so a failure leaves the setting on, as it still is).
///
/// # Errors
///
/// The patch changes background settings or does not fit, the settings
/// file cannot be read or written, or the hints cannot be dropped.
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
        if old.nearby.wol_hints && !new.nearby.wol_hints {
            let platform = platform.as_ref();
            KnownDevices::open(platform)?.clear_wake_hints(platform)?;
        }
        store.save(&new)
    })
    .await
}
