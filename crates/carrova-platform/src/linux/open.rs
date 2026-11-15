//! Open a URL or path with the desktop's default application (`xdg-open`).

use std::ffi::OsStr;
use std::process::{Command, Stdio};

use super::LinuxPlatform;
use crate::PlatformError;

impl LinuxPlatform {
    /// Hand `target` (already checked, see `crate::open`) to `xdg-open`.
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
