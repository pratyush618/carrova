//! Start at sign-in on Windows: a value under this user's
//! `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` key.

use std::io;
use std::path::Path;

use winreg::RegKey;
use winreg::enums::{HKEY_CURRENT_USER, KEY_SET_VALUE};

use super::file_manager::command_path;
use crate::PlatformError;

const RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
/// Our value; the others under `Run` belong to other apps.
const VALUE: &str = "Carrova";

/// Read access only: a policy may allow reading `Run` but not writing it.
pub(super) fn autostart_on() -> Result<bool, PlatformError> {
    let found = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(RUN)
        .and_then(|key| key.get_raw_value(VALUE));
    match found {
        Ok(_) => Ok(true),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(registry_error(e)),
    }
}

pub(super) fn set_autostart(launcher: &Path, on: bool) -> Result<bool, PlatformError> {
    let key = writable_run_key()?;
    let written = if on {
        let exe = command_path(launcher).ok_or(PlatformError::Unsupported("launcher path"))?;
        key.set_value(VALUE, &format!("\"{exe}\" --background"))
    } else {
        match key.delete_value(VALUE) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            other => other,
        }
    };
    written.map_err(registry_error)?;
    autostart_on()
}

fn writable_run_key() -> Result<RegKey, PlatformError> {
    RegKey::predef(HKEY_CURRENT_USER)
        .create_subkey_with_flags(RUN, KEY_SET_VALUE)
        .map(|(key, _)| key)
        .map_err(registry_error)
}

fn registry_error(e: io::Error) -> PlatformError {
    PlatformError::io(Path::new("HKCU").join(RUN).join(VALUE), e)
}
