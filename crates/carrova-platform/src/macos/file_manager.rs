//! "Send with Carrova" Finder Quick Action.
//!
//! One Automator Services workflow under `~/Library/Services`: a Run Shell
//! Script action taking the selected files and folders as arguments and
//! running `open -n -a '<Carrova.app>' --args --send "$@"` for the app bundle
//! holding the launcher, so renamed or moved installs still work. Carrova
//! owns that workflow outright; nothing else in the Services folder is
//! touched. Templates sit beside this module.

use std::io::{self, Write};
use std::path::{Path, PathBuf};

use super::MacPlatform;
use crate::{FileManagerStatus, Platform, PlatformError};

#[cfg(test)]
#[path = "../../tests/macos/file_manager/unit.rs"]
mod tests;

/// The workflow bundle; its name is what macOS shows until the plist is read.
const BUNDLE: &str = "Send with Carrova.workflow";
const SERVICES: &str = "Library/Services";

const INFO_PLIST: &str = include_str!("file_manager/Info.plist");
/// Holds one `{{command}}` placeholder: the shell command, XML-escaped.
const DOCUMENT_WFLOW: &str = include_str!("file_manager/document.wflow");

impl MacPlatform {
    pub(super) fn quick_action_status(&self) -> Result<FileManagerStatus, PlatformError> {
        Ok(FileManagerStatus {
            supported: true,
            installed: std::fs::symlink_metadata(self.services()?.join(BUNDLE)).is_ok(),
            skipped: Vec::new(),
        })
    }

    pub(super) fn install_quick_action(
        &self,
        launcher: &Path,
    ) -> Result<FileManagerStatus, PlatformError> {
        let wflow = DOCUMENT_WFLOW.replace("{{command}}", &xml_escape(&open_command(launcher)?));
        let bundle = self.services()?.join(BUNDLE);
        self.write_atomic(&bundle.join("Contents/Info.plist"), INFO_PLIST)?;
        self.write_atomic(&bundle.join("Contents/document.wflow"), &wflow)?;
        self.quick_action_status()
    }

    pub(super) fn remove_quick_action(&self) -> Result<FileManagerStatus, PlatformError> {
        let services = self.services()?;
        ignore_missing(self.remove_dir_all_confined(&services.join(BUNDLE), &services))?;
        self.quick_action_status()
    }

    fn services(&self) -> Result<PathBuf, PlatformError> {
        Ok(self.home()?.join(SERVICES))
    }

    /// Temp file beside `path`, then rename over it; parents are created.
    fn write_atomic(&self, path: &Path, contents: &str) -> Result<(), PlatformError> {
        let (Some(dir), Some(name)) = (path.parent(), path.file_name()) else {
            return Err(PlatformError::io(path, io::ErrorKind::InvalidInput.into()));
        };
        std::fs::create_dir_all(dir).map_err(|e| PlatformError::io(dir, e))?;
        let mut tmp_name = std::ffi::OsString::from(".");
        tmp_name.push(name);
        tmp_name.push(".carrova-tmp");
        let tmp = dir.join(tmp_name);
        ignore_missing(self.remove_file(&tmp))?;
        let mut file = self.create_new_file(&tmp, Some(0o644))?;
        file.write_all(contents.as_bytes())
            .and_then(|()| file.sync_all())
            .map_err(|e| PlatformError::io(&tmp, e))?;
        self.rename_replace(&tmp, path)?;
        self.sync_dir(dir)
    }
}

fn ignore_missing(result: Result<(), PlatformError>) -> Result<(), PlatformError> {
    match result {
        Err(PlatformError::Io { source, .. }) if source.kind() == io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

/// `open -n -a '<app>' --args --send "$@"` for the nearest `.app` holding
/// `launcher`; a launcher outside any app bundle (a dev build) is refused.
/// `-n` starts a fresh instance even when Carrova runs: plain `open -a`
/// drops `--args` for a running app, while the fresh instance hands its
/// argv to the running one through the single-instance plugin and exits.
fn open_command(launcher: &Path) -> Result<String, PlatformError> {
    let app = launcher
        .ancestors()
        .find(|p| p.extension().is_some_and(|e| e == "app"))
        .and_then(Path::to_str)
        .filter(|a| a.starts_with('/') && !a.chars().any(char::is_control))
        .ok_or(PlatformError::Unsupported("launcher outside an app bundle"))?;
    Ok(format!(
        r#"open -n -a '{}' --args --send "$@""#,
        app.replace('\'', r"'\''")
    ))
}

/// Text inside a plist `<string>` element.
fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
