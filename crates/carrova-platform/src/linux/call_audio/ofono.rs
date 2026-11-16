//! oFono, the hands-free side: is it there, keypad tones, audio link.

use std::collections::HashMap;

use zbus::blocking::Connection;
use zbus::blocking::fdo::DBusProxy;
use zbus::names::BusName;
use zbus::zvariant::{OwnedObjectPath, OwnedValue};

use super::{bus_err, system};
use crate::PlatformError;

pub(super) const SERVICE: &str = "org.ofono";

type Objects = Vec<(OwnedObjectPath, HashMap<String, OwnedValue>)>;

/// Whether oFono runs or can be started on demand.
pub(super) fn present(conn: &Connection) -> zbus::Result<bool> {
    let bus = DBusProxy::new(conn)?;
    Ok(bus.name_has_owner(BusName::try_from(SERVICE)?)?
        || bus
            .list_activatable_names()?
            .iter()
            .any(|n| n.as_str() == SERVICE))
}

/// `method` on oFono's root object, listing objects with properties.
fn list(conn: &Connection, interface: &str, method: &str) -> zbus::Result<Objects> {
    conn.call_method(Some(SERVICE), "/", Some(interface), method, &())?
        .body()
        .deserialize()
}

fn text(props: &HashMap<String, OwnedValue>, key: &str) -> Option<String> {
    props.get(key)?.try_clone().ok()?.try_into().ok()
}

/// `VoiceCallManager.SendTones` on the first modem that has calls.
pub(super) fn send_tones(digits: &str) -> Result<(), PlatformError> {
    let run = || -> zbus::Result<bool> {
        let conn = system()?;
        let modems = list(&conn, "org.ofono.Manager", "GetModems")?;
        let Some((modem, _)) = modems.iter().find(|(_, props)| {
            props
                .get("Interfaces")
                .and_then(|v| Vec::<String>::try_from(v.try_clone().ok()?).ok())
                .is_some_and(|ifaces| ifaces.iter().any(|i| i == "org.ofono.VoiceCallManager"))
        }) else {
            return Ok(false);
        };
        conn.call_method(
            Some(SERVICE),
            modem,
            Some("org.ofono.VoiceCallManager"),
            "SendTones",
            &digits,
        )?;
        Ok(true)
    };
    match run().map_err(|e| bus_err(SERVICE, e))? {
        true => Ok(()),
        false => Err(PlatformError::Unsupported(
            "call_tones: no hands-free modem",
        )),
    }
}

/// `HandsfreeAudioCard.Connect` on the phone's card: the call's audio
/// comes to this computer.
pub(super) fn connect_audio() -> Result<(), PlatformError> {
    let run = || -> zbus::Result<bool> {
        let conn = system()?;
        let cards = list(&conn, "org.ofono.HandsfreeAudioManager", "GetCards")?;
        // The phone is the remote audio gateway.
        let Some((card, _)) = cards
            .iter()
            .find(|(_, props)| text(props, "Type").as_deref() == Some("gateway"))
        else {
            return Ok(false);
        };
        conn.call_method(
            Some(SERVICE),
            card,
            Some("org.ofono.HandsfreeAudioCard"),
            "Connect",
            &(),
        )?;
        Ok(true)
    };
    match run().map_err(|e| bus_err(SERVICE, e))? {
        true => Ok(()),
        false => Err(PlatformError::Unsupported(
            "call_audio_to: no phone audio card",
        )),
    }
}
