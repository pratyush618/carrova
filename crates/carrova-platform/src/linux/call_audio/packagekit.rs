//! Installing oFono through PackageKit (the OS asks for the password), on
//! Ubuntu 24.04 and newer only.

use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant};

use zbus::blocking::{Connection, MessageIterator};
use zbus::message::Type;
use zbus::zvariant::OwnedObjectPath;
use zbus::{MatchRule, Message};

use super::bus_err;
use crate::PlatformError;

const SERVICE: &str = "org.freedesktop.PackageKit";
const PATH: &str = "/org/freedesktop/PackageKit";
const TRANSACTION: &str = "org.freedesktop.PackageKit.Transaction";
const PACKAGE: &str = "ofono";
/// Filters `not-installed | newest | arch` (bits of `PkFilterEnum`).
const RESOLVE_FILTER: u64 = 1 << 3 | 1 << 14 | 1 << 16;
/// `PkTransactionFlagEnum` `only-trusted`.
const ONLY_TRUSTED: u64 = 1 << 1;
/// `PkExitEnum` values.
const EXIT_SUCCESS: u32 = 1;
const EXIT_CANCELLED: u32 = 3;
/// `PkErrorEnum` values that mean the user stopped it (the password
/// prompt dismissed is `not-authorized`).
const ERROR_TRANSACTION_CANCELLED: u32 = 17;
const ERROR_NOT_AUTHORIZED: u32 = 46;
/// The whole install, password prompt and download included.
const INSTALL_DEADLINE: Duration = Duration::from_secs(10 * 60);

/// How a finished transaction ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Outcome {
    /// It succeeded.
    Done,
    /// The user stopped it.
    Cancelled,
    /// It failed.
    Failed,
}

/// The [`Outcome`] of exit code `exit` after error code `error`, if any.
pub(crate) fn outcome(exit: u32, error: Option<u32>) -> Outcome {
    match (exit, error) {
        (EXIT_SUCCESS, _) => Outcome::Done,
        (EXIT_CANCELLED, _) | (_, Some(ERROR_TRANSACTION_CANCELLED | ERROR_NOT_AUTHORIZED)) => {
            Outcome::Cancelled
        }
        _ => Outcome::Failed,
    }
}

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

/// Install oFono; already installed is no error. Gives up (and cancels)
/// after [`INSTALL_DEADLINE`]; a dismissed password prompt is
/// [`PlatformError::Cancelled`].
pub(crate) fn install() -> Result<(), PlatformError> {
    if !supported() {
        return Err(PlatformError::Unsupported("install_call_audio"));
    }
    // No reply timeout: replies are quick, the wait is on signals.
    let conn = Connection::system().map_err(|e| bus_err(SERVICE, e))?;
    let deadline = Instant::now() + INSTALL_DEADLINE;
    let run = || {
        let ids = transaction(&conn, "Resolve", &(RESOLVE_FILTER, [PACKAGE]), deadline)?;
        let Some(id) = ids.into_iter().next() else {
            return Ok(());
        };
        transaction(&conn, "InstallPackages", &(ONLY_TRUSTED, [id]), deadline).map(drop)
    };
    let result = run();
    // Ends the signal threads' iterators.
    let _ = conn.close();
    result
}

/// A signal that matters to a running transaction.
#[derive(Debug)]
enum Event {
    Package(String),
    Error(u32),
    Finished(u32),
    /// PackageKit left the bus.
    Gone,
}

fn transaction_event(msg: &Message) -> Option<Event> {
    let header = msg.header();
    let body = msg.body();
    match header.member()?.as_str() {
        "Package" => {
            let (_info, id, _summary): (u32, String, String) = body.deserialize().ok()?;
            Some(Event::Package(id))
        }
        "ErrorCode" => {
            let (code, _details): (u32, String) = body.deserialize().ok()?;
            Some(Event::Error(code))
        }
        "Finished" => {
            let (exit, _runtime): (u32, u32) = body.deserialize().ok()?;
            Some(Event::Finished(exit))
        }
        _ => None,
    }
}

fn owner_event(msg: &Message) -> Option<Event> {
    let (_name, _old, new): (String, String, String) = msg.body().deserialize().ok()?;
    new.is_empty().then_some(Event::Gone)
}

/// Forward the signals `rule` matches, decoded, from a thread of their own.
fn watch(
    conn: &Connection,
    rule: MatchRule<'_>,
    send: Sender<Event>,
    decode: fn(&Message) -> Option<Event>,
) -> Result<(), PlatformError> {
    let signals =
        MessageIterator::for_match_rule(rule, conn, None).map_err(|e| bus_err(SERVICE, e))?;
    thread::Builder::new()
        .name("packagekit-signals".into())
        .spawn(move || {
            for msg in signals.map_while(Result::ok) {
                if let Some(event) = decode(&msg)
                    && send.send(event).is_err()
                {
                    break;
                }
            }
        })
        .map(drop)
        .map_err(|e| PlatformError::io(SERVICE, e))
}

/// Run `method` in a new transaction until it finishes or `deadline`; the
/// package ids it reported.
fn transaction<B>(
    conn: &Connection,
    method: &'static str,
    args: &B,
    deadline: Instant,
) -> Result<Vec<String>, PlatformError>
where
    B: serde::Serialize + zbus::zvariant::DynamicType,
{
    let bus = |e| bus_err(SERVICE, e);
    let tx: OwnedObjectPath = conn
        .call_method(Some(SERVICE), PATH, Some(SERVICE), "CreateTransaction", &())
        .and_then(|r| r.body().deserialize())
        .map_err(bus)?;
    let signals = MatchRule::builder()
        .msg_type(Type::Signal)
        .sender(SERVICE)
        .and_then(|b| b.path(tx.as_str()))
        .and_then(|b| b.interface(TRANSACTION))
        .map_err(bus)?
        .build();
    let owner = MatchRule::builder()
        .msg_type(Type::Signal)
        .sender("org.freedesktop.DBus")
        .and_then(|b| b.member("NameOwnerChanged"))
        .and_then(|b| b.arg(0, SERVICE))
        .map_err(bus)?
        .build();
    let (send, recv) = mpsc::channel();
    // Subscribed before the call, so no signal is missed.
    watch(conn, signals, send.clone(), transaction_event)?;
    watch(conn, owner, send, owner_event)?;
    conn.call_method(Some(SERVICE), &tx, Some(TRANSACTION), method, args)
        .map_err(bus)?;
    let mut ids = Vec::new();
    let mut error = None;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        match recv.recv_timeout(left) {
            Ok(Event::Package(id)) => ids.push(id),
            Ok(Event::Error(code)) => error = Some(code),
            Ok(Event::Finished(exit)) => {
                return match outcome(exit, error) {
                    Outcome::Done => Ok(ids),
                    Outcome::Cancelled => Err(PlatformError::Cancelled("install_call_audio")),
                    Outcome::Failed => Err(PlatformError::io(
                        SERVICE,
                        std::io::Error::other(format!(
                            "{method} failed: exit {exit}, error {error:?}"
                        )),
                    )),
                };
            }
            Ok(Event::Gone) | Err(RecvTimeoutError::Disconnected) => {
                return Err(PlatformError::io(
                    SERVICE,
                    std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "PackageKit went away"),
                ));
            }
            Err(RecvTimeoutError::Timeout) => {
                let _ = conn.call_method(Some(SERVICE), &tx, Some(TRANSACTION), "Cancel", &());
                return Err(PlatformError::io(
                    SERVICE,
                    std::io::ErrorKind::TimedOut.into(),
                ));
            }
        }
    }
}
