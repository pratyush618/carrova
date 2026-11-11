//! The screensaver services of the session: wake the display and ask
//! whether the screen is locked. Best effort: GNOME answers the lock query
//! on `org.gnome.ScreenSaver` only, others on `org.freedesktop.ScreenSaver`,
//! and a desktop with neither leaves the lock [`ScreenLock::Unknown`].

use std::time::Duration;

use zbus::blocking::{Connection, connection::Builder};

use crate::{PlatformError, ScreenLock};

/// Services asked for the lock state, in order.
const LOCK_SERVICES: [&str; 2] = ["org.gnome.ScreenSaver", "org.freedesktop.ScreenSaver"];
const WAKE_SERVICE: &str = "org.freedesktop.ScreenSaver";

/// How long a screensaver service may take to answer.
const REPLY_TIMEOUT: Duration = Duration::from_secs(1);

fn session() -> zbus::Result<Connection> {
    Builder::session()?.method_timeout(REPLY_TIMEOUT).build()
}

fn path_of(service: &str) -> String {
    format!("/{}", service.replace('.', "/"))
}

/// `SimulateUserActivity`: the same effect as a key press.
pub(super) fn wake() -> Result<(), PlatformError> {
    let call = || {
        session()?.call_method(
            Some(WAKE_SERVICE),
            path_of(WAKE_SERVICE).as_str(),
            Some(WAKE_SERVICE),
            "SimulateUserActivity",
            &(),
        )
    };
    call().map(drop).map_err(|e| PlatformError::Io {
        path: WAKE_SERVICE.into(),
        source: std::io::Error::other(e),
    })
}

/// `GetActive` of `service`: `None` if it does not answer.
fn get_active(service: &str) -> Option<bool> {
    let reply = session()
        .ok()?
        .call_method(
            Some(service),
            path_of(service).as_str(),
            Some(service),
            "GetActive",
            &(),
        )
        .ok()?;
    reply.body().deserialize::<bool>().ok()
}

/// Whether the screen is locked, by the first service that answers.
pub(super) fn lock() -> ScreenLock {
    lock_from(get_active)
}

/// [`lock`] over `ask`, which gives a service's answer (`None`: none).
pub(super) fn lock_from(ask: impl Fn(&str) -> Option<bool>) -> ScreenLock {
    LOCK_SERVICES
        .iter()
        .find_map(|service| ask(service))
        .map_or(ScreenLock::Unknown, |active| {
            if active {
                ScreenLock::Locked
            } else {
                ScreenLock::Unlocked
            }
        })
}
