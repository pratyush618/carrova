//! Facts about this Carrova and this machine that front-ends show ("About")
//! and LAN peers exchange (the receiver's [`crate::lan::Inventory`]).

use carrova_platform::Platform;

/// Version of the engine (this crate).
pub const ENGINE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// This machine's name as the user knows it (e.g. its host name), if the
/// platform can tell: [`Platform::device_name`], already sanitised for
/// display. Callers treat it as optional (screens fall back to "the other
/// computer").
#[must_use]
pub fn device_name(platform: &dyn Platform) -> Option<String> {
    platform.device_name()
}
