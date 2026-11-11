//! "Send with Carrova" entries for Explorer: per-user (no admin) shell
//! verbs for files (`*`) and folders (`Directory`) under
//! `HKCU\Software\Classes`. A multi-selection launches once per path; the
//! running app collects them.

use std::io;
use std::path::Path;

use winreg::RegKey;
use winreg::enums::{HKEY_CURRENT_USER, KEY_ALL_ACCESS};

use crate::{FileManagerStatus, PlatformError};

#[cfg(test)]
#[path = "../../tests/windows/file_manager/unit.rs"]
mod tests;

/// Verb keys we own under the classes root; remove deletes exactly these.
const VERBS: [&str; 2] = [r"*\shell\Carrova", r"Directory\shell\Carrova"];
const LABEL: &str = "Send with Carrova";
const CLASSES: &str = r"Software\Classes";

/// This user's classes root, where the Explorer verbs live.
#[derive(Debug)]
pub(super) struct Entries {
    /// `HKCU\Software\Classes` (a scratch key in tests).
    classes: RegKey,
}

impl Entries {
    /// This user's `HKCU\Software\Classes`.
    pub(super) fn for_user() -> Result<Self, PlatformError> {
        RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey_with_flags(CLASSES, KEY_ALL_ACCESS)
            .map(Self::at)
            .map_err(|e| registry_error("", e))
    }

    fn at(classes: RegKey) -> Self {
        Self { classes }
    }

    pub(super) fn status(&self) -> Result<FileManagerStatus, PlatformError> {
        let mut installed = false;
        for verb in VERBS {
            installed |= match self.classes.open_subkey(verb) {
                Ok(_) => true,
                Err(e) if e.kind() == io::ErrorKind::NotFound => false,
                Err(e) => return Err(registry_error(verb, e)),
            };
        }
        Ok(FileManagerStatus {
            supported: true,
            installed,
            skipped: Vec::new(),
        })
    }

    pub(super) fn install(&self, launcher: &Path) -> Result<FileManagerStatus, PlatformError> {
        let exe = command_path(launcher).ok_or(PlatformError::Unsupported("launcher path"))?;
        let command = format!("\"{exe}\" --send \"%1\"");
        for verb in VERBS {
            self.write_verb(verb, exe, &command)
                .map_err(|e| registry_error(verb, e))?;
        }
        self.status()
    }

    pub(super) fn remove(&self) -> Result<FileManagerStatus, PlatformError> {
        for verb in VERBS {
            match self.classes.delete_subkey_all(verb) {
                Err(e) if e.kind() != io::ErrorKind::NotFound => {
                    return Err(registry_error(verb, e));
                }
                _ => {}
            }
        }
        self.status()
    }

    fn write_verb(&self, verb: &str, exe: &str, command: &str) -> io::Result<()> {
        let (key, _) = self.classes.create_subkey(verb)?;
        key.set_value("", &LABEL)?;
        key.set_value("Icon", &exe)?;
        // Without it Explorer hides the verb for more than 15 selected items.
        key.set_value("MultiSelectModel", &"Player")?;
        key.create_subkey("command")?.0.set_value("", &command)
    }
}

fn registry_error(key: &str, e: io::Error) -> PlatformError {
    PlatformError::io(Path::new("HKCU").join(CLASSES).join(key), e)
}

/// `launcher` as text for the quoted registry command: absolute, and free
/// of `"` and control characters, which would break the quoting.
fn command_path(launcher: &Path) -> Option<&str> {
    let exe = launcher.to_str()?;
    (launcher.is_absolute() && !exe.chars().any(|c| c == '"' || c.is_control())).then_some(exe)
}
