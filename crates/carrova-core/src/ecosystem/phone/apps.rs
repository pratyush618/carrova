//! Which phone apps may forward notifications, and the filters applied to a
//! note before it is sent. Stored in `<state>/phone/notify-apps.json`
//! (`600`, atomic). App ids and note contents are never logged.

use std::collections::BTreeMap;

use carrova_platform::Platform;
use carrova_platform::phone::{Hidden, Note};
use serde::{Deserialize, Serialize};

use super::otp;
use crate::error::{CoreError, Corruption, EncodeError, Result};
use crate::store::{ensure_dir, read_file, write_atomic};

const FILE: &str = "notify-apps.json";
const VERSION: u16 = 1;
/// Apps that default to off (`data/private-apps.txt`).
const PRIVATE_APPS: &str = include_str!("../../../data/private-apps.txt");

const fn yes() -> bool {
    true
}

/// The user's per-app notification choices and the two content filters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotifyApps {
    version: u16,
    /// Explicit choices by package id; an unlisted app uses its default.
    apps: BTreeMap<String, bool>,
    /// Withhold notifications that carry a one-time code.
    #[serde(default = "yes")]
    hide_codes: bool,
    /// Send message text (off: title only).
    #[serde(default = "yes")]
    show_text: bool,
}

impl Default for NotifyApps {
    fn default() -> Self {
        Self {
            version: VERSION,
            apps: BTreeMap::new(),
            hide_codes: true,
            show_text: true,
        }
    }
}

impl NotifyApps {
    /// The saved choices; the defaults if none were saved.
    ///
    /// # Errors
    /// The file cannot be read, or is corrupt or from a newer version.
    pub fn open(platform: &dyn Platform) -> Result<Self> {
        let Some(bytes) = read_file(platform, &dir(platform)?.join(FILE))? else {
            return Ok(Self::default());
        };
        let corrupt = |reason| CoreError::CorruptFile {
            file: FILE.to_owned(),
            reason,
        };
        let me: Self = serde_json::from_slice(&bytes).map_err(|e| corrupt(e.into()))?;
        if me.version > VERSION {
            return Err(corrupt(Corruption::Version(me.version)));
        }
        Ok(me)
    }

    /// Whether `app_id` may forward: its saved choice, else on unless it is
    /// a banking, payment, password or authenticator app.
    #[must_use]
    pub fn allowed(&self, app_id: &str) -> bool {
        self.apps
            .get(app_id)
            .copied()
            .unwrap_or_else(|| !is_private(app_id))
    }

    /// Turn `app_id` on or off and save.
    ///
    /// # Errors
    /// The file cannot be written; the choice is then not kept.
    pub fn set(&mut self, platform: &dyn Platform, app_id: &str, on: bool) -> Result<()> {
        self.edit(platform, |me| {
            me.apps.insert(app_id.to_owned(), on);
        })
    }

    #[must_use]
    pub fn hide_codes(&self) -> bool {
        self.hide_codes
    }

    #[must_use]
    pub fn show_text(&self) -> bool {
        self.show_text
    }

    /// # Errors
    /// The file cannot be written.
    pub fn set_hide_codes(&mut self, platform: &dyn Platform, on: bool) -> Result<()> {
        self.edit(platform, |me| me.hide_codes = on)
    }

    /// # Errors
    /// The file cannot be written.
    pub fn set_show_text(&mut self, platform: &dyn Platform, on: bool) -> Result<()> {
        self.edit(platform, |me| me.show_text = on)
    }

    /// Apply `change`, save, and keep it only if the save worked.
    fn edit(&mut self, platform: &dyn Platform, change: impl FnOnce(&mut Self)) -> Result<()> {
        let mut next = self.clone();
        change(&mut next);
        let bytes = serde_json::to_vec_pretty(&next).map_err(|e| CoreError::Encode {
            what: "notification apps".to_owned(),
            source: EncodeError::from(e),
        })?;
        write_atomic(platform, &dir(platform)?, FILE, &bytes)?;
        *self = next;
        Ok(())
    }
}

/// `<state>/phone`, made `700` if missing.
pub(super) fn dir(platform: &dyn Platform) -> Result<std::path::PathBuf> {
    let dir = platform.state_dir()?.join("phone");
    ensure_dir(platform, &dir, 0o700)?;
    Ok(dir)
}

fn is_private(app_id: &str) -> bool {
    PRIVATE_APPS
        .lines()
        .map(str::trim)
        .any(|l| !l.is_empty() && !l.starts_with('#') && l == app_id)
}

/// What may leave the phone: `None` if the app is off; a code is withheld
/// from title and text; with text off only the title goes.
pub(super) fn outgoing(mut note: Note, apps: &NotifyApps) -> Option<Note> {
    if !apps.allowed(&note.app_id) {
        return None;
    }
    if apps.hide_codes && (otp::looks_like_code(&note.text) || otp::looks_like_code(&note.title)) {
        note.title.clear();
        note.text.clear();
        note.hidden = Some(Hidden::Code);
    } else if !apps.show_text {
        note.text.clear();
        note.hidden = Some(Hidden::TextOff);
    }
    Some(note)
}

#[cfg(test)]
#[path = "../../../tests/ecosystem/phone/apps/unit.rs"]
mod tests;
