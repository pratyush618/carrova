//! The user's clipboard settings (spec "Clipboard": history in memory plus
//! an opt-in file) and the files behind them, in the state directory:
//! `clipboard.json` and, only while [`ClipSettings::history_file`] is on,
//! `clip-history.cbor`. Both `600`, written atomically. Contents are never
//! logged.
//!
//! shortcut: the whole history is rewritten on each clip, and a history
//! over the 64 MiB document limit (many large images) is not read back;
//! append or cap images if users keep big image histories.

use std::path::PathBuf;

use carrova_link::{ClipHistory, ReceivedClip};
use carrova_platform::Platform;
use serde::{Deserialize, Serialize};

use crate::error::{CoreError, Corruption, EncodeError, Result};
use crate::fsx;
use crate::store::{ensure_dir, read_file, write_atomic};

/// Settings file name in the state directory.
const SETTINGS_FILE: &str = "clipboard.json";
/// History file name in the state directory.
const HISTORY_FILE: &str = "clip-history.cbor";

/// How this user's clipboard is shared (Settings → Clipboard). Missing
/// fields read as the defaults.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(default)]
pub struct ClipSettings {
    /// Send each copy here to linked devices that take the clipboard. Off,
    /// only an explicit "Send clipboard" sends it.
    pub share: bool,
    /// Show a note when a device's clipboard arrives
    /// ([`crate::Event::ClipReceived`] `note`).
    pub show_note: bool,
    /// Also keep the received history in a file, so it outlives a
    /// restart. Off, it stays in memory and the file is deleted.
    pub history_file: bool,
}

impl Default for ClipSettings {
    fn default() -> Self {
        Self {
            share: true,
            show_note: false,
            history_file: false,
        }
    }
}

/// Where the clipboard files live.
fn state_dir(platform: &dyn Platform) -> Result<PathBuf> {
    let dir = platform.state_dir()?;
    ensure_dir(platform, &dir, 0o700)?;
    Ok(dir)
}

fn corrupt(file: &str, reason: Corruption) -> CoreError {
    CoreError::CorruptFile {
        file: file.to_owned(),
        reason,
    }
}

/// The saved settings; the defaults if none were saved.
pub(super) fn load_settings(platform: &dyn Platform) -> Result<ClipSettings> {
    let path = state_dir(platform)?.join(SETTINGS_FILE);
    match read_file(platform, &path)? {
        Some(bytes) => {
            serde_json::from_slice(&bytes).map_err(|e| corrupt(SETTINGS_FILE, Corruption::from(e)))
        }
        None => Ok(ClipSettings::default()),
    }
}

pub(super) fn save_settings(platform: &dyn Platform, settings: &ClipSettings) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(settings).map_err(|e| CoreError::Encode {
        what: "clipboard settings".to_owned(),
        source: EncodeError::from(e),
    })?;
    write_atomic(platform, &state_dir(platform)?, SETTINGS_FILE, &bytes)
}

/// The history saved in the file (empty if there is none).
pub(super) fn load_history(platform: &dyn Platform) -> Result<ClipHistory> {
    let path = state_dir(platform)?.join(HISTORY_FILE);
    let mut history = ClipHistory::default();
    if let Some(bytes) = read_file(platform, &path)? {
        // Never logged: a decode error may quote a clip.
        let clips: Vec<ReceivedClip> = ciborium::from_reader(bytes.as_slice())
            .map_err(|e| corrupt(HISTORY_FILE, Corruption::from(e)))?;
        // Saved newest first; pushed oldest first.
        for clip in clips.into_iter().rev() {
            history.push(clip.item, clip.at);
        }
    }
    Ok(history)
}

pub(super) fn save_history(platform: &dyn Platform, history: &ClipHistory) -> Result<()> {
    let clips: Vec<&ReceivedClip> = history.items().collect();
    let mut bytes = Vec::new();
    ciborium::into_writer(&clips, &mut bytes).map_err(|e| CoreError::Encode {
        what: "clipboard history".to_owned(),
        source: EncodeError::from(e),
    })?;
    write_atomic(platform, &state_dir(platform)?, HISTORY_FILE, &bytes)
}

/// Delete the history file; nothing if there is none.
pub(super) fn remove_history(platform: &dyn Platform) -> Result<()> {
    fsx::remove_any(platform, &state_dir(platform)?.join(HISTORY_FILE)).map(drop)
}
