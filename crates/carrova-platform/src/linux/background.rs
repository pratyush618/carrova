//! Background mode on Linux: start at sign-in through an XDG autostart
//! entry, and status-tray detection on the session bus (a StatusNotifier
//! host must be registered for a tray icon to show).

use std::path::{Path, PathBuf};

use zbus::zvariant::OwnedValue;

use super::LinuxPlatform;
use super::file_manager::{exec_launcher, exec_quote, key_string, remove_if_exists};
use super::screen::session;
use crate::{PlatformError, SemanticRoot};

#[cfg(test)]
#[path = "../../tests/linux/background/unit.rs"]
mod tests;

/// Our entry under the user's config dir; named after the app id.
const AUTOSTART: &str = "autostart/org.byteveda.Carrova.desktop";
const WATCHER: &str = "org.kde.StatusNotifierWatcher";

impl LinuxPlatform {
    pub(super) fn autostart_on(&self) -> Result<bool, PlatformError> {
        Ok(std::fs::symlink_metadata(self.autostart_path()?).is_ok())
    }

    pub(super) fn write_autostart(&self, launcher: &Path, on: bool) -> Result<bool, PlatformError> {
        let path = self.autostart_path()?;
        if on {
            self.write_atomic(&path, &entry(exec_launcher(launcher)?), 0o644)?;
        } else {
            remove_if_exists(self, &path)?;
        }
        self.autostart_on()
    }

    fn autostart_path(&self) -> Result<PathBuf, PlatformError> {
        Ok(self.root_path(SemanticRoot::UserConfig)?.join(AUTOSTART))
    }
}

/// The autostart entry for a launcher [`exec_launcher`] accepted.
fn entry(launcher: &str) -> String {
    format!(
        "[Desktop Entry]\nType=Application\nName=Carrova\nExec={} --background\n\
         TryExec={}\nIcon=carrova-desktop\nNoDisplay=true\nX-GNOME-Autostart-enabled=true\n",
        exec_quote(launcher),
        key_string(launcher),
    )
}

/// Whether the watcher is on the session bus and reports a registered host.
/// No bus, no watcher or no answer within the bus timeout: no tray.
pub(super) fn status_tray() -> bool {
    let registered = || -> zbus::Result<bool> {
        let reply = session()?.call_method(
            Some(WATCHER),
            "/StatusNotifierWatcher",
            Some("org.freedesktop.DBus.Properties"),
            "Get",
            &(WATCHER, "IsStatusNotifierHostRegistered"),
        )?;
        Ok(bool::try_from(reply.body().deserialize::<OwnedValue>()?)?)
    };
    registered().unwrap_or(false)
}
