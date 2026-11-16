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
/// How long pairing may wait for the phone to show up and be confirmed.
const PAIR_TIMEOUT: Duration = Duration::from_secs(60);
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

/// BlueZ's state for [`super::status_from`].
pub(super) fn snapshot(conn: &Connection) -> zbus::Result<Snapshot> {
    let objects = objects(conn)?;
    let Some((_, props)) = adapter(&objects) else {
        return Ok(Snapshot::default());
    };
    Ok(Snapshot {
        adapter: Some(AdapterInfo {
            powered: flag(props, "Powered"),
        }),
        hf_profile: has_uuid(props, HFP_HF),
        paired_phone: with(&objects, DEVICE).any(|(_, d)| flag(d, "Paired") && has_uuid(d, HFP_AG)),
    })
}

/// The device to pair among `(path, paired, icon)`: the only unpaired
/// phone in range; none while there are several (the user would have to
/// pick, and confirming the wrong one must not be possible).
pub(crate) fn pair_target<'a>(
    devices: impl IntoIterator<Item = (&'a str, bool, Option<&'a str>)>,
) -> Option<&'a str> {
    let mut phones = devices
        .into_iter()
        .filter(|&(_, paired, icon)| !paired && icon == Some("phone"))
        .map(|(path, _, _)| path);
    let first = phones.next()?;
    phones.next().is_none().then_some(first)
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
/// screens show; gives up after [`PAIR_TIMEOUT`].
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
    match result.map_err(bus)? {
        true => Ok(()),
        false => Err(PlatformError::io(
            SERVICE,
            std::io::ErrorKind::TimedOut.into(),
        )),
    }
}

/// Discover until one phone shows up, then `Pair` and `Trusted`; `false`
/// on timeout.
fn pair_with(conn: &Connection, target: &Mutex<Option<OwnedObjectPath>>) -> zbus::Result<bool> {
    let deadline = Instant::now() + PAIR_TIMEOUT;
    let adapter_path = adapter(&objects(conn)?)
        .map(|(path, _)| path.clone())
        .ok_or_else(|| zbus::Error::Failure("no Bluetooth adapter".into()))?;
    let adapter_call =
        |method: &str| conn.call_method(Some(SERVICE), &adapter_path, Some(ADAPTER), method, &());
    adapter_call("StartDiscovery")?;
    let found = loop {
        let objects = objects(conn)?;
        let devices = with(&objects, DEVICE).map(|(path, d)| {
            let icon = d.get("Icon").and_then(|v| v.downcast_ref::<&str>().ok());
            (path.as_str(), flag(d, "Paired"), icon)
        });
        if let Some(path) = pair_target(devices) {
            break Some(OwnedObjectPath::try_from(path)?);
        }
        if Instant::now() >= deadline {
            break None;
        }
        thread::sleep(POLL);
    };
    let _ = adapter_call("StopDiscovery");
    let Some(device) = found else {
        return Ok(false);
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
    Ok(true)
}
