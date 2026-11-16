//! Call audio: this computer as a Bluetooth Hands-Free unit for the phone,
//! which stays the audio gateway. BlueZ pairs, oFono runs the hands-free
//! role (keypad, audio link), PipeWire carries the sound once WirePlumber
//! hands HFP to oFono. BlueZ, oFono and PackageKit are on the system bus.
//!
//! Never logged: device names, addresses, numbers, keypad digits.

use std::io;

use zbus::blocking::{Connection, connection::Builder};

use super::LinuxPlatform;
use crate::phone::{AudioSide, CallAudioStatus};
use crate::{PlatformError, SemanticRoot};

mod bluez;
mod ofono;
mod packagekit;
mod wireplumber;

#[cfg(test)]
#[path = "../../../tests/linux/call_audio/unit.rs"]
mod tests;

pub(super) use bluez::pair;
pub(super) use packagekit::install;

/// How long a system service may take to answer a quick query.
const REPLY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);

/// The Bluetooth adapter, as far as call audio cares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AdapterInfo {
    /// Bluetooth is switched on.
    pub(crate) powered: bool,
}

/// The call-audio state, first blocker first.
pub(crate) fn status_from(
    adapter: Option<AdapterInfo>,
    hf_profile: bool,
    ofono_present: bool,
    paired_phone: bool,
) -> CallAudioStatus {
    match adapter {
        None => CallAudioStatus::NoAdapter,
        Some(AdapterInfo { powered: false }) => CallAudioStatus::BluetoothOff,
        // oFono registers the hands-free role, so it is checked first.
        Some(_) if !ofono_present => CallAudioStatus::NeedsPackage,
        Some(_) if !hf_profile => CallAudioStatus::NoHandsFree,
        Some(_) if !paired_phone => CallAudioStatus::NeedsPairing,
        Some(_) => CallAudioStatus::Ready,
    }
}

/// DTMF digits oFono may send: `0`–`9`, `*`, `#`, at least one.
pub(crate) fn valid_tones(digits: &str) -> bool {
    !digits.is_empty()
        && digits
            .chars()
            .all(|c| c.is_ascii_digit() || c == '*' || c == '#')
}

/// The system bus, with replies bounded by [`REPLY_TIMEOUT`].
fn system() -> zbus::Result<Connection> {
    Builder::system()?.method_timeout(REPLY_TIMEOUT).build()
}

/// A bus failure on `service`, by error name only: BlueZ and oFono
/// messages may carry a device path, which holds the device's address.
fn bus_err(service: &'static str, e: zbus::Error) -> PlatformError {
    let cause = match e {
        zbus::Error::MethodError(name, _, _) => name.to_string(),
        other => other.to_string(),
    };
    PlatformError::io(service, io::Error::other(cause))
}

/// Current call-audio state.
pub(super) fn status() -> Result<CallAudioStatus, PlatformError> {
    let conn = system().map_err(|e| bus_err("system bus", e))?;
    let bt = bluez::snapshot(&conn).map_err(|e| bus_err(bluez::SERVICE, e))?;
    let ofono = ofono::present(&conn).map_err(|e| bus_err(ofono::SERVICE, e))?;
    let status = status_from(bt.adapter, bt.hf_profile, ofono, bt.paired_phone);
    // Only supported systems get the install screen.
    if status == CallAudioStatus::NeedsPackage && !packagekit::supported() {
        return Ok(CallAudioStatus::Unsupported);
    }
    Ok(status)
}

/// Send keypad `digits` on the active call.
pub(super) fn tones(digits: &str) -> Result<(), PlatformError> {
    if !valid_tones(digits) {
        return Err(PlatformError::InvalidInput("call tones"));
    }
    ofono::send_tones(digits)
}

/// Route the active call's audio to `side`.
pub(super) fn audio_to(side: AudioSide) -> Result<(), PlatformError> {
    match side {
        AudioSide::Laptop => ofono::connect_audio(),
        // shortcut: oFono's HandsfreeAudioCard has Connect but no Disconnect,
        // so the phone takes the audio back itself; add once spike S4 finds a way.
        AudioSide::Phone => Err(PlatformError::Unsupported("call_audio_to phone")),
    }
}

impl LinuxPlatform {
    /// Write (`on`) or remove the WirePlumber drop-in that hands HFP to
    /// oFono, then restart the user's WirePlumber.
    pub(super) fn call_audio_config(&self, on: bool) -> Result<(), PlatformError> {
        if on {
            let version = wireplumber::installed_version()
                .ok_or(PlatformError::Unsupported("wireplumber"))?;
            wireplumber::write(self, &version)?;
        } else {
            wireplumber::remove(self)?;
        }
        wireplumber::restart()
    }

    fn config_dir(&self) -> Result<std::path::PathBuf, PlatformError> {
        self.root_path(SemanticRoot::UserConfig)
    }
}
