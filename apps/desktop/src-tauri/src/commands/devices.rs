//! Nearby, known-device and visibility commands.

use carrova_view::devices::{
    ForgetDevices, KnownDeviceView, NearbyDeviceView, VisibilityChoice, VisibilityView,
};
use tauri::State;

use crate::command_error::CommandError;
use crate::ops;
use crate::state::AppState;

/// `CarrovaPort.nearby`.
#[tauri::command]
pub async fn nearby(state: State<'_, AppState>) -> Result<Vec<NearbyDeviceView>, CommandError> {
    ops::devices::nearby(&state).await
}

/// `CarrovaPort.knownDevices`.
#[tauri::command]
pub async fn known_devices(
    state: State<'_, AppState>,
) -> Result<Vec<KnownDeviceView>, CommandError> {
    ops::devices::known_devices(&state).await
}

/// `CarrovaPort.renameDevice`.
#[tauri::command]
pub async fn rename_device(
    state: State<'_, AppState>,
    device: String,
    name: String,
) -> Result<(), CommandError> {
    ops::devices::rename_device(&state, device, name).await
}

/// `CarrovaPort.forgetDevice`.
#[tauri::command]
pub async fn forget_device(
    state: State<'_, AppState>,
    which: ForgetDevices,
) -> Result<(), CommandError> {
    ops::devices::forget_device(&state, which).await
}

/// `CarrovaPort.resetIdentity`.
#[tauri::command]
pub async fn reset_identity(state: State<'_, AppState>) -> Result<(), CommandError> {
    ops::devices::reset_identity(&state).await
}

/// `CarrovaPort.visibility`.
#[tauri::command]
pub async fn visibility(state: State<'_, AppState>) -> Result<VisibilityView, CommandError> {
    ops::devices::visibility(&state).await
}

/// `CarrovaPort.setVisibility`.
#[tauri::command]
pub async fn set_visibility(
    state: State<'_, AppState>,
    choice: VisibilityChoice,
) -> Result<VisibilityView, CommandError> {
    ops::devices::set_visibility(&state, choice).await
}
