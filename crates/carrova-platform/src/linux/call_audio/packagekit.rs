//! Installing oFono through PackageKit (the OS asks for the password), on
//! Ubuntu 24.04 and newer only.

use zbus::MatchRule;
use zbus::blocking::{Connection, MessageIterator};
use zbus::message::Type;
use zbus::zvariant::OwnedObjectPath;

use super::bus_err;
use crate::PlatformError;

const SERVICE: &str = "org.freedesktop.PackageKit";
const PATH: &str = "/org/freedesktop/PackageKit";
const TRANSACTION: &str = "org.freedesktop.PackageKit.Transaction";
const PACKAGE: &str = "ofono";
/// Filters `not-installed | newest | arch` (bits of `PkFilterEnum`).
const RESOLVE_FILTER: u64 = 1 << 3 | 1 << 16 | 1 << 18;
/// `PkTransactionFlagEnum` `only-trusted`.
const ONLY_TRUSTED: u64 = 1 << 1;
/// `PkExitEnum` `success`.
const EXIT_SUCCESS: u32 = 1;

/// Whether `os_release` (the text of `/etc/os-release`) is Ubuntu 24.04 or
/// newer.
pub(crate) fn supported_os(os_release: &str) -> bool {
    let field = |key: &str| {
        os_release.lines().find_map(|l| {
            let value = l.strip_prefix(key)?.strip_prefix('=')?;
            Some(value.trim().trim_matches('"'))
        })
    };
    let version = field("VERSION_ID").and_then(|v| {
        let (major, minor) = v.split_once('.')?;
        Some((major.parse::<u32>().ok()?, minor.parse::<u32>().ok()?))
    });
    field("ID") == Some("ubuntu") && version.is_some_and(|v| v >= (24, 4))
}

/// [`supported_os`] for this machine.
pub(super) fn supported() -> bool {
    ["/etc/os-release", "/usr/lib/os-release"]
        .iter()
        .find_map(|p| std::fs::read_to_string(p).ok())
        .is_some_and(|text| supported_os(&text))
}

/// Install oFono; already installed is no error.
pub(crate) fn install() -> Result<(), PlatformError> {
    if !supported() {
        return Err(PlatformError::Unsupported("install_call_audio"));
    }
    let run = || -> Result<(), PlatformError> {
        // No reply timeout: the install waits on the user's password.
        let conn = Connection::system().map_err(|e| bus_err(SERVICE, e))?;
        let ids = transaction(&conn, "Resolve", &(RESOLVE_FILTER, [PACKAGE]))?;
        let Some(id) = ids.into_iter().next() else {
            return Ok(());
        };
        transaction(&conn, "InstallPackages", &(ONLY_TRUSTED, [id])).map(drop)
    };
    run()
}

/// Run `method` in a new transaction until it finishes; the package ids it
/// reported.
fn transaction<B>(conn: &Connection, method: &str, args: &B) -> Result<Vec<String>, PlatformError>
where
    B: serde::Serialize + zbus::zvariant::DynamicType,
{
    let bus = |e| bus_err(SERVICE, e);
    let tx: OwnedObjectPath = conn
        .call_method(Some(SERVICE), PATH, Some(SERVICE), "CreateTransaction", &())
        .and_then(|r| r.body().deserialize())
        .map_err(bus)?;
    let rule = MatchRule::builder()
        .msg_type(Type::Signal)
        .sender(SERVICE)
        .and_then(|b| b.path(tx.as_str()))
        .and_then(|b| b.interface(TRANSACTION))
        .map_err(bus)?
        .build();
    // Subscribed before the call, so no signal is missed.
    let signals = MessageIterator::for_match_rule(rule, conn, None).map_err(bus)?;
    conn.call_method(Some(SERVICE), &tx, Some(TRANSACTION), method, args)
        .map_err(bus)?;
    let mut ids = Vec::new();
    for msg in signals {
        let msg = msg.map_err(bus)?;
        let member = msg.header().member().map(ToString::to_string);
        match member.as_deref() {
            Some("Package") => {
                let (_info, id, _summary): (u32, String, String) =
                    msg.body().deserialize().map_err(bus)?;
                ids.push(id);
            }
            Some("Finished") => {
                let (exit, _runtime): (u32, u32) = msg.body().deserialize().map_err(bus)?;
                return if exit == EXIT_SUCCESS {
                    Ok(ids)
                } else {
                    Err(PlatformError::io(
                        SERVICE,
                        std::io::Error::other(format!("{method} ended with exit {exit}")),
                    ))
                };
            }
            _ => {}
        }
    }
    Err(PlatformError::io(
        SERVICE,
        std::io::ErrorKind::UnexpectedEof.into(),
    ))
}
