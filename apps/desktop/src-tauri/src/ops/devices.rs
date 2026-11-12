//! Nearby devices, known devices and who may find this computer. Each
//! change to visibility or the known devices refreshes the presence advert
//! ([`crate::presence`]).

use carrova_core::devices::KnownDevices;
use carrova_core::lan::browse_nearby;
use carrova_core::presence::{load_visibility, save_visibility};
use carrova_core::time::now_unix;
use carrova_view::devices::{
    ForgetDevices, KnownDeviceView, NearbyDeviceView, VisibilityChoice, VisibilityView,
    chosen_visibility, known_device_views, nearby_views, visibility_view,
};

use crate::command_error::CommandError;
use crate::state::AppState;
use crate::worker::blocking;

/// Every Carrova computer nearby: known devices, devices shown to
/// everyone and senders offering a code.
///
/// # Errors
///
/// Discovery could not start.
pub async fn nearby(state: &AppState) -> Result<Vec<NearbyDeviceView>, CommandError> {
    let platform = state.platform.clone();
    blocking(move || Ok(nearby_views(&browse_nearby(platform.as_ref())?))).await
}

/// The devices this computer remembers.
///
/// # Errors
///
/// The known-device store cannot be read.
pub async fn known_devices(state: &AppState) -> Result<Vec<KnownDeviceView>, CommandError> {
    let platform = state.platform.clone();
    blocking(move || {
        let platform = platform.as_ref();
        Ok(known_device_views(
            &KnownDevices::open(platform)?.list(platform)?,
        ))
    })
    .await
}

/// Give the known device `device` (id or fingerprint) a new local name.
///
/// # Errors
///
/// No such device, several match, the name is empty once cleaned, or the
/// store cannot be written.
pub async fn rename_device(
    state: &AppState,
    device: String,
    name: String,
) -> Result<(), CommandError> {
    let platform = state.platform.clone();
    blocking(move || {
        let platform = platform.as_ref();
        let store = KnownDevices::open(platform)?;
        Ok(store.rename(platform, &store.resolve_id(platform, &device)?, &name)?)
    })
    .await
}

/// Forget one known device (id or fingerprint), or all of them.
///
/// # Errors
///
/// No such device, several match, or the store cannot be written.
pub async fn forget_device(state: &AppState, which: ForgetDevices) -> Result<(), CommandError> {
    let shared = state.clone();
    blocking(move || {
        let platform = shared.platform.as_ref();
        let store = KnownDevices::open(platform)?;
        match which {
            ForgetDevices::All => store.forget_all(platform)?,
            ForgetDevices::Device(device) => {
                store.forget(platform, &store.resolve_id(platform, &device)?)?;
            }
        }
        shared.presence.refresh();
        Ok(())
    })
    .await
}

/// Forget every device and give this computer a new identity.
///
/// # Errors
///
/// The store or the new key cannot be written.
pub async fn reset_identity(state: &AppState) -> Result<(), CommandError> {
    let shared = state.clone();
    blocking(move || {
        let platform = shared.platform.as_ref();
        KnownDevices::open(platform)?.reset_identity(platform)?;
        shared.presence.refresh();
        Ok(())
    })
    .await
}

/// Wake the known device `device` (id or fingerprint) with a Wake-on-LAN
/// packet from its saved hint. Best effort: it may not wake.
///
/// # Errors
///
/// No such device, no wake hint saved for it, or the packet could not be
/// sent.
pub async fn wake_device(state: &AppState, device: String) -> Result<(), CommandError> {
    let platform = state.platform.clone();
    blocking(move || {
        let platform = platform.as_ref();
        let store = KnownDevices::open(platform)?;
        Ok(store.wake(platform, &store.resolve_id(platform, &device)?)?)
    })
    .await
}

/// Who may find this computer now.
///
/// # Errors
///
/// The saved visibility cannot be read.
pub async fn visibility(state: &AppState) -> Result<VisibilityView, CommandError> {
    let platform = state.platform.clone();
    blocking(move || {
        Ok(visibility_view(load_visibility(
            platform.as_ref(),
            now_unix(),
        )?))
    })
    .await
}

/// Let `choice` find this computer from now on; replies with the
/// visibility in force.
///
/// # Errors
///
/// The visibility cannot be saved.
pub async fn set_visibility(
    state: &AppState,
    choice: VisibilityChoice,
) -> Result<VisibilityView, CommandError> {
    let shared = state.clone();
    blocking(move || apply_visibility(&shared, choice)).await
}

/// [`set_visibility`], blocking (for the tray menu).
///
/// # Errors
///
/// The visibility cannot be saved.
pub fn apply_visibility(
    state: &AppState,
    choice: VisibilityChoice,
) -> Result<VisibilityView, CommandError> {
    let visibility = chosen_visibility(choice, now_unix());
    save_visibility(state.platform.as_ref(), visibility)?;
    state.presence.refresh();
    Ok(visibility_view(visibility))
}
