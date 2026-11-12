//! Wake-on-LAN for a known device (ADR 0001: the hardware address is a
//! saved hint, never logged and never in an error).

use carrova_crypto::identity::{DeviceId, Fingerprint};
use carrova_platform::Platform;

use super::KnownDevices;
use crate::error::{CoreError, Result};

impl KnownDevices {
    /// Broadcast a magic packet for the known device `id`, using the
    /// hardware address saved when it was remembered. Best effort: a sent
    /// packet says nothing about the device waking.
    /// [`CoreError::UnknownDevice`] if it is not known,
    /// [`CoreError::NoWakeHint`] if no address was saved.
    pub fn wake(&self, platform: &dyn Platform, id: &DeviceId) -> Result<()> {
        let record = self
            .list(platform)?
            .into_iter()
            .find(|d| d.public == *id)
            .ok_or(CoreError::UnknownDevice)?;
        let mac = record.mac_hint.ok_or(CoreError::NoWakeHint)?;
        platform.send_magic_packet(mac)?;
        tracing::debug!(device = %Fingerprint::of(id), "sent a wake packet");
        Ok(())
    }
}
