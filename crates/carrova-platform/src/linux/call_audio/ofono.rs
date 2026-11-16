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
pub(super) fn installed(conn: &Connection) -> zbus::Result<bool> {
    Ok(running(conn)?
        || DBusProxy::new(conn)?
            .list_activatable_names()?
            .iter()
            .any(|n| n.as_str() == SERVICE))
}

/// Whether oFono runs now.
pub(super) fn running(conn: &Connection) -> zbus::Result<bool> {
    Ok(DBusProxy::new(conn)?.name_has_owner(BusName::try_from(SERVICE)?)?)
}

/// Start oFono through bus activation.
pub(super) fn start(conn: &Connection) -> zbus::Result<()> {
    DBusProxy::new(conn)?.start_service_by_name(SERVICE.try_into()?, 0)?;
    Ok(())
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

/// The card among `(path, type, remote address)` whose remote is the
/// audio gateway at `address`.
pub(crate) fn card_for<'a>(
    cards: impl IntoIterator<Item = (&'a str, Option<&'a str>, Option<&'a str>)>,
    address: &str,
) -> Option<&'a str> {
    cards
        .into_iter()
        .find(|&(_, kind, remote)| {
            kind == Some("gateway") && remote.is_some_and(|r| r.eq_ignore_ascii_case(address))
        })
        .map(|(path, _, _)| path)
}

/// The phone's (at `address`) audio card, while its hands-free link is up.
fn card(conn: &Connection, address: &str) -> zbus::Result<Option<OwnedObjectPath>> {
    let cards = list(conn, "org.ofono.HandsfreeAudioManager", "GetCards")?;
    let props: Vec<_> = cards
        .iter()
        .map(|(path, p)| (path.as_str(), text(p, "Type"), text(p, "RemoteAddress")))
        .collect();
    let found = card_for(
        props
            .iter()
            .map(|(path, kind, remote)| (*path, kind.as_deref(), remote.as_deref())),
        address,
    );
    found
        .map(OwnedObjectPath::try_from)
        .transpose()
        .map_err(Into::into)
}

/// Whether the phone at `address` has a hands-free link (an audio card).
pub(super) fn has_card(conn: &Connection, address: &str) -> zbus::Result<bool> {
    Ok(card(conn, address)?.is_some())
}

/// How long the audio card may take to appear after the link comes up.
const CARD_WAIT: std::time::Duration = std::time::Duration::from_secs(3);

/// `HandsfreeAudioCard.Connect` on the card of the phone at `address`:
/// the call's audio comes to this computer.
pub(super) fn connect_audio(conn: &Connection, address: &str) -> Result<(), PlatformError> {
    let run = || -> zbus::Result<bool> {
        let deadline = std::time::Instant::now() + CARD_WAIT;
        let card = loop {
            match card(conn, address)? {
                Some(card) => break card,
                None if std::time::Instant::now() >= deadline => return Ok(false),
                None => std::thread::sleep(std::time::Duration::from_millis(200)),
            }
        };
        conn.call_method(
            Some(SERVICE),
            &card,
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
