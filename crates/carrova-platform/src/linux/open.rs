//! Open a URL or path with the desktop's default application (`xdg-open`).

use std::ffi::OsStr;
use std::path::Path;
use std::process::{Command, Stdio};

use super::LinuxPlatform;
use crate::PlatformError;

#[cfg(test)]
#[path = "../../tests/linux/open/unit.rs"]
mod tests;

/// `url` if it is `http(s)`: anything else could start any program.
pub(super) fn checked_url(url: &str) -> Result<&OsStr, PlatformError> {
    let web = ["http://", "https://"].iter().any(|scheme| {
        url.get(..scheme.len())
            .is_some_and(|head| head.eq_ignore_ascii_case(scheme))
    });
    if web {
        Ok(OsStr::new(url))
    } else {
        Err(PlatformError::InvalidInput(
            "only http(s) URLs may be opened",
        ))
    }
}

/// `path` if absolute: a relative one could read as an option or a URL.
pub(super) fn checked_path(path: &Path) -> Result<&OsStr, PlatformError> {
    if path.is_absolute() {
        Ok(path.as_os_str())
    } else {
        Err(PlatformError::InvalidInput(
            "only absolute paths may be opened",
        ))
    }
}

impl LinuxPlatform {
    /// Hand `target` (already checked) to `xdg-open`.
    pub(super) fn xdg_open(&self, target: &OsStr) -> Result<(), PlatformError> {
        let bin = crate::sys::find_binary(self.env.path.as_deref(), "xdg-open")
            .ok_or(PlatformError::Unsupported("xdg-open is not installed"))?;
        // Not waited for: `xdg-open` may stay for as long as the app runs.
        let mut child = Command::new(&bin)
            .arg(target)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| PlatformError::io(&bin, e))?;
        std::thread::spawn(move || child.wait());
        Ok(())
    }
}
