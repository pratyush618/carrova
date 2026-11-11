//! The screensaver service of the session (`org.freedesktop.ScreenSaver`):
//! wake the display and ask whether the screen is locked. Both are best
//! effort; a desktop without the service answers with an error or `false`.

use zbus::blocking::Connection;

use crate::PlatformError;

const SERVICE: &str = "org.freedesktop.ScreenSaver";
const PATH: &str = "/org/freedesktop/ScreenSaver";

fn call<R>(method: &str, read: impl FnOnce(zbus::Message) -> zbus::Result<R>) -> zbus::Result<R> {
    let reply =
        Connection::session()?.call_method(Some(SERVICE), PATH, Some(SERVICE), method, &())?;
    read(reply)
}

/// `SimulateUserActivity`: the same effect as a key press.
pub(super) fn wake() -> Result<(), PlatformError> {
    call("SimulateUserActivity", |_| Ok(())).map_err(|e| PlatformError::Io {
        path: SERVICE.into(),
        source: std::io::Error::other(e),
    })
}

/// `GetActive`: whether the screensaver or lock screen is showing.
pub(super) fn locked() -> bool {
    call("GetActive", |reply| reply.body().deserialize::<bool>()).unwrap_or(false)
}
