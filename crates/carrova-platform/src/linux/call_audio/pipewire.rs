//! PipeWire: whether the phone's hands-free audio link is up. PipeWire is
//! oFono's audio agent and owns the audio socket, so the link is read from
//! its graph (`pw-dump`) rather than taken from oFono.

use std::process::Command;

use serde_json::Value;

/// Most bytes `pw-dump` may print (a desktop's graph is a few hundred KiB).
const DUMP_CAP: usize = 8 * 1024 * 1024;

/// PipeWire's graph as `pw-dump` prints it; `None` if it can't be read.
pub(super) fn dump() -> Option<String> {
    crate::tool::stdout_capped(&mut Command::new("pw-dump"), DUMP_CAP)
}

/// Whether `dump` holds a Bluetooth hands-free node for the device at
/// `address`.
// shortcut: unverified on hardware until spike S4; assumes the node exists
// only while the audio link is up and takes either HFP/HSP profile name
// (PipeWire names it after the phone's role). Check `info.state` in S4.
pub(super) fn hands_free_up(dump: &str, address: &str) -> bool {
    let Ok(Value::Array(objects)) = serde_json::from_str::<Value>(dump) else {
        return false;
    };
    objects.iter().any(|o| {
        let props = &o["info"]["props"];
        o["type"] == "PipeWire:Interface:Node"
            && props["api.bluez5.address"]
                .as_str()
                .is_some_and(|a| a.eq_ignore_ascii_case(address))
            && matches!(
                props["api.bluez5.profile"].as_str(),
                Some("headset-head-unit" | "headset-audio-gateway")
            )
    })
}
