//! What the About sheet shows.

use carrova_core::about::{ENGINE_VERSION, device_name};
use carrova_view::ipc::About;

use crate::state::AppState;

/// This app's and engine's versions, the device name (if the platform
/// can tell), the OS and whether it shows a status tray (a session-bus
/// call on Linux: run it off the async runtime).
pub fn about(state: &AppState) -> About {
    About {
        app_version: env!("CARGO_PKG_VERSION").to_owned(),
        engine_version: ENGINE_VERSION.to_owned(),
        device_name: device_name(state.platform.as_ref()),
        os: state.platform.os(),
        status_tray: state.platform.has_status_tray(),
    }
}
