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

/// How long a system service may take to answer a quick query.
const REPLY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);
/// How long BlueZ may take to bring the phone's hands-free link up or down.
const LINK_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

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
    configured: bool,
    paired_phone: bool,
) -> CallAudioStatus {
    match adapter {
        None => CallAudioStatus::NoAdapter,
        Some(AdapterInfo { powered: false }) => CallAudioStatus::BluetoothOff,
        // oFono (set up by the drop-in) registers the hands-free role, so a
        // missing role only counts against the hardware once both are there.
        Some(_) if !ofono_present || !configured => CallAudioStatus::NeedsPackage,
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
    use zbus::DBusError;
    let cause = match e {
        zbus::Error::MethodError(name, _, _) => name.to_string(),
        zbus::Error::FDO(fdo) => fdo.name().to_string(),
        other => other.to_string(),
    };
    PlatformError::io(service, io::Error::other(cause))
}

/// How long oFono may take to register the hands-free role once started.
const OFONO_SETTLE: std::time::Duration = std::time::Duration::from_secs(1);

/// Current call-audio state; starts oFono when it is installed but idle.
fn status(configured: bool) -> Result<CallAudioStatus, PlatformError> {
    let conn = system().map_err(|e| bus_err("system bus", e))?;
    let snapshot = || bluez::snapshot(&conn).map_err(|e| bus_err(bluez::SERVICE, e));
    let ofono_err = |e| bus_err(ofono::SERVICE, e);
    let mut bt = snapshot()?;
    let mut ofono = ofono::installed(&conn).map_err(ofono_err)?;
    let powered = bt.adapter.is_some_and(|a| a.powered);
    if ofono
        && configured
        && powered
        && !bt.hf_profile
        && !ofono::running(&conn).map_err(ofono_err)?
    {
        // A role missing only because oFono is idle is no hardware fault.
        ofono = ofono::start(&conn).is_ok() && ofono::running(&conn).map_err(ofono_err)?;
        std::thread::sleep(OFONO_SETTLE);
        bt = snapshot()?;
    }
    let status = status_from(
        bt.adapter,
        bt.hf_profile,
        ofono,
        configured,
        bt.paired_phone,
    );
    // Only systems PackageKit may install oFono on get the install screen.
    if status == CallAudioStatus::NeedsPackage && !ofono && !packagekit::supported() {
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
    // Bringing the hands-free link up takes a few seconds.
    let conn = Builder::system()
        .and_then(|b| b.method_timeout(LINK_TIMEOUT).build())
        .map_err(|e| bus_err("system bus", e))?;
    let (device, address) = bluez::phone(&conn)
        .map_err(|e| bus_err(bluez::SERVICE, e))?
        .ok_or(PlatformError::Unsupported("call_audio_to: no paired phone"))?;
    match side {
        // shortcut: verify in S4 that Android takes the call back to its
        // earpiece when the hands-free link drops.
        AudioSide::Phone => bluez::profile(&conn, &device, false),
        // shortcut: verify in S4 that the audio card shows up once the
        // hands-free link is back.
        AudioSide::Laptop => {
            if !ofono::has_card(&conn, &address).map_err(|e| bus_err(ofono::SERVICE, e))? {
                bluez::profile(&conn, &device, true)?;
            }
            ofono::connect_audio(&conn, &address)
        }
    }
}

impl LinuxPlatform {
    /// [`status`], with the drop-in checked for the installed WirePlumber.
    pub(super) fn call_audio_status_impl(&self) -> Result<CallAudioStatus, PlatformError> {
        let configured = match wireplumber::installed_version() {
            Some(version) => wireplumber::configured(self, &version)?,
            None => false,
        };
        status(configured)
    }

    /// Install oFono if it is missing, then hand HFP to it.
    pub(super) fn install_call_audio_impl(&self) -> Result<(), PlatformError> {
        let conn = system().map_err(|e| bus_err("system bus", e))?;
        if !ofono::installed(&conn).map_err(|e| bus_err(ofono::SERVICE, e))? {
            packagekit::install()?;
        }
        self.call_audio_config(true)
    }

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
