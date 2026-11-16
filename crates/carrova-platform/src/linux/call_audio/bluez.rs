//! BlueZ: the adapter's state, the paired phone, and pairing it with a
//! passkey agent.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use zbus::blocking::{Connection, connection::Builder};
use zbus::zvariant::{ObjectPath, OwnedObjectPath, OwnedValue, Value};

use super::{AdapterInfo, bus_err};
use crate::PlatformError;

pub(super) const SERVICE: &str = "org.bluez";
const ADAPTER: &str = "org.bluez.Adapter1";
const DEVICE: &str = "org.bluez.Device1";
/// Service class UUIDs: Hands-Free (our role) and its Audio Gateway (the phone).
const HFP_HF: &str = "0000111e-0000-1000-8000-00805f9b34fb";
const HFP_AG: &str = "0000111f-0000-1000-8000-00805f9b34fb";
const AGENT_PATH: &str = "/org/byteveda/Carrova/agent";
/// How long pairing looks for the phone, then waits for the user to
/// confirm on it: about a minute in all.
const DISCOVERY_TIMEOUT: Duration = Duration::from_secs(30);
const PAIR_TIMEOUT: Duration = Duration::from_secs(30);
const POLL: Duration = Duration::from_secs(1);

type Props = HashMap<String, OwnedValue>;
type Objects = HashMap<OwnedObjectPath, HashMap<String, Props>>;

/// What BlueZ reports for call audio.
#[derive(Debug, Default)]
pub(super) struct Snapshot {
    pub(super) adapter: Option<AdapterInfo>,
    /// The hands-free role is registered on the adapter.
    pub(super) hf_profile: bool,
    /// A paired device offers the audio gateway role.
    pub(super) paired_phone: bool,
}

/// All BlueZ objects; none when BlueZ is not running.
fn objects(conn: &Connection) -> zbus::Result<Objects> {
    let reply = conn.call_method(
        Some(SERVICE),
        "/",
        Some("org.freedesktop.DBus.ObjectManager"),
        "GetManagedObjects",
        &(),
    );
    match reply {
        Ok(reply) => reply.body().deserialize(),
        Err(zbus::Error::MethodError(name, _, _))
            if name.as_str() == "org.freedesktop.DBus.Error.ServiceUnknown" =>
        {
            Ok(Objects::new())
        }
        Err(e) => Err(e),
    }
}

fn flag(props: &Props, key: &str) -> bool {
    props
        .get(key)
        .and_then(|v| v.downcast_ref::<bool>().ok())
        .unwrap_or(false)
}

fn has_uuid(props: &Props, uuid: &str) -> bool {
    props
        .get("UUIDs")
        .and_then(|v| Vec::<String>::try_from(v.try_clone().ok()?).ok())
        .is_some_and(|uuids| uuids.iter().any(|u| u.eq_ignore_ascii_case(uuid)))
}

fn with<'a>(
    objects: &'a Objects,
    iface: &'a str,
) -> impl Iterator<Item = (&'a OwnedObjectPath, &'a Props)> {
    objects
        .iter()
        .filter_map(move |(path, ifaces)| Some((path, ifaces.get(iface)?)))
}

/// The adapter to use: a powered one if any.
fn adapter(objects: &Objects) -> Option<(&OwnedObjectPath, &Props)> {
    let mut adapters: Vec<_> = with(objects, ADAPTER).collect();
    adapters.sort_by_key(|(path, props)| (!flag(props, "Powered"), path.as_str()));
    adapters.into_iter().next()
}

/// Whether the device object at `device` hangs off the adapter at
/// `adapter` (`/org/bluez/hci0` owns `/org/bluez/hci0/dev_…`).
pub(crate) fn on_adapter(adapter: &str, device: &str) -> bool {
    device
        .strip_prefix(adapter)
        .is_some_and(|rest| rest.starts_with('/'))
}

/// The device objects on the adapter at `adapter`.
fn devices_on<'a>(
    objects: &'a Objects,
    adapter: &'a OwnedObjectPath,
) -> impl Iterator<Item = (&'a OwnedObjectPath, &'a Props)> {
    with(objects, DEVICE).filter(|(path, _)| on_adapter(adapter.as_str(), path.as_str()))
}

/// BlueZ's state for [`super::status_from`].
pub(super) fn snapshot(conn: &Connection) -> zbus::Result<Snapshot> {
    let objects = objects(conn)?;
    let Some((adapter_path, props)) = adapter(&objects) else {
        return Ok(Snapshot::default());
    };
    Ok(Snapshot {
        adapter: Some(AdapterInfo {
            powered: flag(props, "Powered"),
        }),
        hf_profile: has_uuid(props, HFP_HF),
        paired_phone: devices_on(&objects, adapter_path)
            .any(|(_, d)| flag(d, "Paired") && has_uuid(d, HFP_AG)),
    })
}

/// What discovery found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Seen<T> {
    /// No phone to pair.
    None,
    /// Exactly one.
    One(T),
    /// Several: the user would have to pick, and confirming the wrong one
    /// must not be possible.
    Several,
}

/// The device to pair among `(path, paired, icon)`: the only unpaired
/// phone in range.
pub(crate) fn pair_target<'a>(
    devices: impl IntoIterator<Item = (&'a str, bool, Option<&'a str>)>,
) -> Seen<&'a str> {
    let mut phones = devices
        .into_iter()
        .filter(|&(_, paired, icon)| !paired && icon == Some("phone"))
        .map(|(path, _, _)| path);
    match (phones.next(), phones.next()) {
        (None, _) => Seen::None,
        (Some(one), None) => Seen::One(one),
        (Some(_), Some(_)) => Seen::Several,
    }
}

/// The phone carrying call audio among `(path, paired with the audio
/// gateway role, connected, address)`: a connected one first.
// shortcut: picks any paired gateway, not the linked phone by name or
// address; targeting it needs a trait change (deferred to a later task).
pub(crate) fn linked_phone<'a>(
    devices: impl IntoIterator<Item = (&'a str, bool, bool, &'a str)>,
) -> Option<(&'a str, &'a str)> {
    devices
        .into_iter()
        .filter(|&(_, gateway, _, _)| gateway)
        .min_by_key(|&(_, _, connected, _)| !connected)
        .map(|(path, _, _, address)| (path, address))
}

/// The paired phone's device object and address.
pub(super) fn phone(conn: &Connection) -> zbus::Result<Option<(OwnedObjectPath, String)>> {
    let objects = objects(conn)?;
    let Some((adapter_path, _)) = adapter(&objects) else {
        return Ok(None);
    };
    let devices: Vec<_> = devices_on(&objects, adapter_path)
        .map(|(path, d)| {
            let address = d.get("Address").and_then(|v| v.downcast_ref::<&str>().ok());
            (
                path.as_str(),
                flag(d, "Paired") && has_uuid(d, HFP_AG),
                flag(d, "Connected"),
                address.unwrap_or_default(),
            )
        })
        .collect();
    let Some((path, address)) = linked_phone(devices) else {
        return Ok(None);
    };
    Ok(Some((OwnedObjectPath::try_from(path)?, address.to_owned())))
}

/// Bring the hands-free link to the phone up (`connect`) or down.
pub(super) fn profile(
    conn: &Connection,
    device: &OwnedObjectPath,
    connect: bool,
) -> Result<(), PlatformError> {
    let method = if connect {
        "ConnectProfile"
    } else {
        "DisconnectProfile"
    };
    conn.call_method(Some(SERVICE), device, Some(DEVICE), method, &HFP_AG)
        .map(drop)
        .map_err(|e| bus_err(SERVICE, e))
}

/// The BlueZ agent: shows the passkey for the device being paired, rejects
/// every other request.
struct Agent {
    target: Arc<Mutex<Option<OwnedObjectPath>>>,
    on_passkey: Mutex<Box<dyn Fn(u32) + Send>>,
}

impl Agent {
    fn show(&self, device: &ObjectPath<'_>, passkey: u32) -> zbus::fdo::Result<()> {
        let target = self.target.lock().unwrap_or_else(PoisonError::into_inner);
        if target
            .as_ref()
            .is_some_and(|t| t.as_str() == device.as_str())
        {
            (self
                .on_passkey
                .lock()
                .unwrap_or_else(PoisonError::into_inner))(passkey);
            Ok(())
        } else {
            Err(zbus::fdo::Error::AccessDenied(
                "not the device being paired".into(),
            ))
        }
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "zbus hands method arguments over by value"
)]
#[zbus::interface(name = "org.bluez.Agent1")]
impl Agent {
    fn release(&self) {}

    fn cancel(&self) {}

    fn display_passkey(
        &self,
        device: ObjectPath<'_>,
        passkey: u32,
        _entered: u16,
    ) -> zbus::fdo::Result<()> {
        self.show(&device, passkey)
    }

    /// Numeric comparison: the user checks the number and confirms on the
    /// phone, which is what pairs.
    fn request_confirmation(&self, device: ObjectPath<'_>, passkey: u32) -> zbus::fdo::Result<()> {
        self.show(&device, passkey)
    }
}

/// Pair and trust the phone, calling `on_passkey` with the code both
/// screens show; gives up after about a minute.
///
/// No phone found: an I/O error of kind `TimedOut`. Several phones
/// visible until the end: kind `ResourceBusy` ("make only your phone
/// visible").
pub(crate) fn pair(on_passkey: Box<dyn Fn(u32) + Send>) -> Result<(), PlatformError> {
    let bus = |e| bus_err(SERVICE, e);
    // `Pair` returns once the user confirms on the phone.
    let conn = Builder::system()
        .and_then(|b| b.method_timeout(PAIR_TIMEOUT).build())
        .map_err(bus)?;
    let target = Arc::new(Mutex::new(None));
    let agent = Agent {
        target: Arc::clone(&target),
        on_passkey: Mutex::new(on_passkey),
    };
    conn.object_server().at(AGENT_PATH, agent).map_err(bus)?;
    let manager = |method: &str| {
        conn.call_method(
            Some(SERVICE),
            "/org/bluez",
            Some("org.bluez.AgentManager1"),
            method,
            &ObjectPath::from_static_str_unchecked(AGENT_PATH),
        )
    };
    conn.call_method(
        Some(SERVICE),
        "/org/bluez",
        Some("org.bluez.AgentManager1"),
        "RegisterAgent",
        &(
            ObjectPath::from_static_str_unchecked(AGENT_PATH),
            "DisplayYesNo",
        ),
    )
    .map_err(bus)?;
    let result = manager("RequestDefaultAgent").and_then(|_| pair_with(&conn, &target));
    let _ = manager("UnregisterAgent");
    let fail = |kind, text: &str| PlatformError::io(SERVICE, std::io::Error::new(kind, text));
    match result.map_err(bus)? {
        Seen::One(()) => Ok(()),
        Seen::None => Err(fail(std::io::ErrorKind::TimedOut, "no phone found")),
        Seen::Several => Err(fail(
            std::io::ErrorKind::ResourceBusy,
            "several phones visible",
        )),
    }
}

/// Discover until one phone shows up, then `Pair` and `Trusted`.
fn pair_with(conn: &Connection, target: &Mutex<Option<OwnedObjectPath>>) -> zbus::Result<Seen<()>> {
    let adapter_path = adapter(&objects(conn)?)
        .map(|(path, _)| path.clone())
        .ok_or_else(|| zbus::Error::Failure("no Bluetooth adapter".into()))?;
    let adapter_call =
        |method: &str| conn.call_method(Some(SERVICE), &adapter_path, Some(ADAPTER), method, &());
    match adapter_call("StartDiscovery") {
        // Someone else is discovering already: that serves us too.
        Err(zbus::Error::MethodError(name, _, _))
            if name.as_str() == "org.bluez.Error.InProgress" => {}
        other => drop(other?),
    }
    let seen = discover(conn, &adapter_path);
    let _ = adapter_call("StopDiscovery");
    let device = match seen? {
        Seen::One(device) => device,
        Seen::None => return Ok(Seen::None),
        Seen::Several => return Ok(Seen::Several),
    };
    *target.lock().unwrap_or_else(PoisonError::into_inner) = Some(device.clone());
    conn.call_method(Some(SERVICE), &device, Some(DEVICE), "Pair", &())?;
    conn.call_method(
        Some(SERVICE),
        &device,
        Some("org.freedesktop.DBus.Properties"),
        "Set",
        &(DEVICE, "Trusted", Value::from(true)),
    )?;
    Ok(Seen::One(()))
}

/// Poll `adapter`'s devices until exactly one phone shows up, or
/// [`DISCOVERY_TIMEOUT`]; then what was seen last.
fn discover(conn: &Connection, adapter: &OwnedObjectPath) -> zbus::Result<Seen<OwnedObjectPath>> {
    let deadline = Instant::now() + DISCOVERY_TIMEOUT;
    loop {
        let objects = objects(conn)?;
        let devices = devices_on(&objects, adapter).map(|(path, d)| {
            let icon = d.get("Icon").and_then(|v| v.downcast_ref::<&str>().ok());
            (path.as_str(), flag(d, "Paired"), icon)
        });
        let seen = pair_target(devices);
        if let Seen::One(path) = seen {
            return Ok(Seen::One(OwnedObjectPath::try_from(path)?));
        }
        if Instant::now() >= deadline {
            return Ok(match seen {
                Seen::Several => Seen::Several,
                _ => Seen::None,
            });
        }
        thread::sleep(POLL);
    }
}
