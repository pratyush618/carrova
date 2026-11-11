//! "Send with Carrova" entries for Linux file managers.
//!
//! Linux has no single file-manager menu standard, so one entry is written
//! per common file manager, each launching `<launcher> --send <paths>`.
//! No "Open With" fallback: a per-user `MimeType=inode/directory` entry can
//! become the default folder opener where no `mimeapps.list` default exists. Carrova owns those files outright; Thunar's
//! `uca.xml` is shared with the user and only our action is touched there
//! (see `thunar`). Templates sit beside this module.

use std::io::{self, Write};
use std::path::{Path, PathBuf};

use super::LinuxPlatform;
use crate::{FileManagerStatus, Platform, PlatformError, SemanticRoot};

#[cfg(test)]
#[path = "../../tests/linux/file_manager/unit.rs"]
mod tests;
mod thunar;

const NAUTILUS_EXTENSION: &str = include_str!("file_manager/carrova_send.py");
const NAUTILUS_SCRIPT: &str = include_str!("file_manager/send_with_carrova.sh");
const NEMO_ACTION: &str = include_str!("file_manager/carrova-send.nemo_action");
const DOLPHIN_MENU: &str = include_str!("file_manager/dolphin.desktop");
const FILE_MANAGER_ACTION: &str = include_str!("file_manager/file_manager_action.desktop");
const THUNAR_ACTION: &str = include_str!("file_manager/thunar_action.xml");

/// A file Carrova owns outright, under the user's data dir.
struct Entry {
    rel: &'static str,
    template: &'static str,
    mode: u32,
}

/// Every file we write besides Thunar's shared `uca.xml`; remove deletes
/// exactly these.
const ENTRIES: [Entry; 5] = [
    Entry {
        rel: "nautilus-python/extensions/carrova_send.py",
        template: NAUTILUS_EXTENSION,
        mode: 0o644,
    },
    Entry {
        rel: "nautilus/scripts/Send with Carrova",
        template: NAUTILUS_SCRIPT,
        mode: 0o755,
    },
    Entry {
        rel: "nemo/actions/carrova-send.nemo_action",
        template: NEMO_ACTION,
        mode: 0o644,
    },
    // Dolphin ignores service menus that are not executable.
    Entry {
        rel: "kio/servicemenus/carrova-send.desktop",
        template: DOLPHIN_MENU,
        mode: 0o755,
    },
    // Caja (caja-actions) and PCManFM-Qt.
    Entry {
        rel: "file-manager/actions/carrova-send.desktop",
        template: FILE_MANAGER_ACTION,
        mode: 0o644,
    },
];

const UCA_REL: &str = "Thunar/uca.xml";
const UCA_MALFORMED: &str = "Thunar: uca.xml is not valid; left unchanged";
const UCA_LINK: &str = "Thunar: uca.xml is a link; left unchanged";

/// What we found in (or did to) Thunar's `uca.xml`.
enum Uca {
    Missing,
    Skipped(&'static str),
    Text(String),
}

impl LinuxPlatform {
    /// `$APPIMAGE` when running from an AppImage (the image, not its
    /// mount), else this executable.
    pub(super) fn launcher_path(&self) -> Result<PathBuf, PlatformError> {
        match self.env.appimage.as_ref().filter(|a| !a.is_empty()) {
            Some(image) => Ok(PathBuf::from(image)),
            None => {
                std::env::current_exe().map_err(|e| PlatformError::io("<current executable>", e))
            }
        }
    }

    pub(super) fn entries_status(&self) -> Result<FileManagerStatus, PlatformError> {
        let data = self.root_path(SemanticRoot::UserData)?;
        let mut installed = false;
        for entry in &ENTRIES {
            installed |= std::fs::symlink_metadata(data.join(entry.rel)).is_ok();
        }
        let mut skipped = Vec::new();
        match self.read_uca()? {
            Uca::Text(text) => installed |= thunar::has_action(&text),
            Uca::Skipped(why) => skipped.push(why.to_owned()),
            Uca::Missing => {}
        }
        Ok(FileManagerStatus {
            supported: true,
            installed,
            skipped,
        })
    }

    pub(super) fn install_entries(
        &self,
        launcher: &Path,
    ) -> Result<FileManagerStatus, PlatformError> {
        let launcher = exec_launcher(launcher)?;
        let data = self.root_path(SemanticRoot::UserData)?;
        for entry in &ENTRIES {
            self.write_atomic(
                &data.join(entry.rel),
                &render(entry.template, launcher),
                entry.mode,
            )?;
        }
        let action = render(THUNAR_ACTION, launcher);
        self.edit_uca(|text| match text {
            Some(text) => thunar::with_action(text, &action).map(Some),
            None => Ok(Some(thunar::new_file(&action))),
        })?;
        self.entries_status()
    }

    pub(super) fn remove_entries(&self) -> Result<FileManagerStatus, PlatformError> {
        let data = self.root_path(SemanticRoot::UserData)?;
        for entry in &ENTRIES {
            remove_if_exists(self, &data.join(entry.rel))?;
        }
        self.edit_uca(|text| text.map(thunar::without_action).transpose())?;
        self.entries_status()
    }

    fn uca_path(&self) -> Result<PathBuf, PlatformError> {
        Ok(self.root_path(SemanticRoot::UserConfig)?.join(UCA_REL))
    }

    fn read_uca(&self) -> Result<Uca, PlatformError> {
        let path = self.uca_path()?;
        match std::fs::symlink_metadata(&path) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Uca::Missing),
            Err(e) => return Err(PlatformError::io(path, e)),
            Ok(meta) if meta.is_symlink() => return Ok(Uca::Skipped(UCA_LINK)),
            Ok(meta) if !meta.is_file() => return Ok(Uca::Skipped(UCA_MALFORMED)),
            Ok(_) => {}
        }
        match std::fs::read_to_string(&path) {
            Ok(text) if thunar::without_action(&text).is_ok() => Ok(Uca::Text(text)),
            Ok(_) => Ok(Uca::Skipped(UCA_MALFORMED)),
            Err(e) if e.kind() == io::ErrorKind::InvalidData => Ok(Uca::Skipped(UCA_MALFORMED)),
            Err(e) => Err(PlatformError::io(path, e)),
        }
    }

    /// Rewrite `uca.xml` with `edit(current text)`; `Ok(None)` leaves it as
    /// is. A file we cannot safely edit is never written.
    fn edit_uca(
        &self,
        edit: impl FnOnce(Option<&str>) -> Result<Option<String>, thunar::Malformed>,
    ) -> Result<(), PlatformError> {
        let path = self.uca_path()?;
        let (current, mode) = match self.read_uca()? {
            Uca::Skipped(_) => return Ok(()),
            Uca::Missing => (None, 0o644),
            Uca::Text(text) => (Some(text), self.metadata(&path)?.mode.unwrap_or(0o644)),
        };
        match edit(current.as_deref()) {
            Ok(Some(text)) if Some(&text) != current.as_ref() => {
                self.write_atomic(&path, &text, mode)
            }
            // Unreachable for text `read_uca` accepted; never write on doubt.
            Ok(_) | Err(thunar::Malformed) => Ok(()),
        }
    }

    /// Temp file beside `path`, then rename over it; parents are created.
    pub(super) fn write_atomic(
        &self,
        path: &Path,
        contents: &str,
        mode: u32,
    ) -> Result<(), PlatformError> {
        let (Some(dir), Some(name)) = (path.parent(), path.file_name()) else {
            return Err(PlatformError::io(path, io::ErrorKind::InvalidInput.into()));
        };
        std::fs::create_dir_all(dir).map_err(|e| PlatformError::io(dir, e))?;
        let mut tmp_name = std::ffi::OsString::from(".");
        tmp_name.push(name);
        tmp_name.push(".carrova-tmp");
        let tmp = dir.join(tmp_name);
        remove_if_exists(self, &tmp)?;
        let mut file = self.create_new_file(&tmp, Some(mode))?;
        file.write_all(contents.as_bytes())
            .and_then(|()| file.sync_all())
            .map_err(|e| PlatformError::io(&tmp, e))?;
        self.rename_replace(&tmp, path)?;
        self.sync_dir(dir)
    }
}

pub(super) fn remove_if_exists(platform: &LinuxPlatform, path: &Path) -> Result<(), PlatformError> {
    match platform.remove_file(path) {
        Err(PlatformError::Io { source, .. }) if source.kind() == io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

/// `launcher` as text a desktop entry's `Exec` may start: absolute, with
/// no `%` (not every file manager un-doubles `%%` in `Exec`: Nemo) and no
/// control characters.
pub(super) fn exec_launcher(launcher: &Path) -> Result<&str, PlatformError> {
    launcher
        .to_str()
        .filter(|l| l.starts_with('/') && !l.chars().any(|c| c == '%' || c.is_control()))
        .ok_or(PlatformError::Unsupported("launcher path"))
}

/// Fill `{{key}}` placeholders in one pass, so a launcher path that itself
/// looks like a placeholder is never expanded.
fn render(template: &str, launcher: &str) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some((key, tail)) = after.split_once("}}") else {
            break;
        };
        match placeholder(key, launcher) {
            Some(value) => out.push_str(&value),
            None => out.push_str(&rest[start..start + key.len() + 4]),
        }
        rest = tail;
    }
    out.push_str(rest);
    out
}

fn placeholder(key: &str, launcher: &str) -> Option<String> {
    Some(match key {
        "exec" => exec_quote(launcher),
        "path" => key_string(launcher),
        "path_list" => key_list(launcher),
        "py" => py_string(launcher),
        "sh" => sh_quote(launcher),
        "thunar" => xml_escape(&thunar_command(launcher)),
        _ => return None,
    })
}

/// One Desktop Entry `Exec` argument: always double-quoted with `"`, `` ` ``,
/// `$` and `\` backslash-escaped and `%` doubled, then key-file escaped.
pub(super) fn exec_quote(arg: &str) -> String {
    let mut quoted = String::from('"');
    for c in arg.chars() {
        match c {
            '"' | '`' | '$' | '\\' => {
                quoted.push('\\');
                quoted.push(c);
            }
            '%' => quoted.push_str("%%"),
            _ => quoted.push(c),
        }
    }
    quoted.push('"');
    key_string(&quoted)
}

/// A key-file string value (the launcher has no control characters).
pub(super) fn key_string(value: &str) -> String {
    value.replace('\\', "\\\\")
}

/// A one-element key-file list value.
fn key_list(value: &str) -> String {
    format!("{};", key_string(value).replace(';', "\\;"))
}

/// A double-quoted Python string literal.
fn py_string(value: &str) -> String {
    let mut literal = String::from('"');
    for c in value.chars() {
        match c {
            '"' | '\\' => {
                literal.push('\\');
                literal.push(c);
            }
            c if c.is_control() => literal.push_str(&format!("\\x{:02x}", u32::from(c))),
            _ => literal.push(c),
        }
    }
    literal.push('"');
    literal
}

/// A single-quoted POSIX shell word.
fn sh_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', r"'\''"))
}

/// Thunar's command line (before XML escaping): it substitutes `%F` and
/// splits like a shell, so the launcher rides in `$0` and is checked first.
fn thunar_command(launcher: &str) -> String {
    format!(
        r#"sh -c 'test -x "$0" && exec "$0" --send "$@"' {} %F"#,
        sh_quote(launcher).replace('%', "%%")
    )
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
