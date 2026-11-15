//! The Linux clipboard: Wayland through the data-control protocol
//! (`wl-clipboard-rs`, which lists MIME types, so a password manager's
//! hint is seen), X11 text through `arboard`.

use std::collections::HashSet;
use std::io::Read;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::Duration;

use wl_clipboard_rs::copy::{self, Options, Source};
use wl_clipboard_rs::paste::{self, ClipboardType, Seat};

use super::LinuxPlatform;
use crate::{ClipContent, ClipKind, ClipboardWatch, PlatformError};

#[cfg(test)]
#[path = "../../tests/linux/clipboard/unit.rs"]
mod tests;

/// A password manager marks a copy secret by offering this MIME type with
/// the content [`SECRET`].
const HINT_MIME: &str = "x-kde-passwordManagerHint";
const SECRET: &[u8] = b"secret";
const TEXT_MIME: &str = "text/plain;charset=utf-8";
const PNG_MIME: &str = "image/png";
/// A clipboard owner could stream without end: refuse more than this.
const MAX_BYTES: u64 = 64 << 20;
/// How often a watch looks at the clipboard.
const POLL: Duration = Duration::from_millis(500);

/// Which clipboard the session has.
#[derive(Debug, Clone, Copy)]
pub(super) enum Clipboard {
    Wayland,
    X11,
}

impl LinuxPlatform {
    pub(super) fn clipboard(&self) -> Clipboard {
        if self.env.wayland_display.is_some() {
            Clipboard::Wayland
        } else {
            Clipboard::X11
        }
    }
}

impl Clipboard {
    pub(super) fn read(self) -> Result<Option<ClipContent>, PlatformError> {
        match self {
            Self::Wayland => read_wayland(),
            Self::X11 => read_x11(),
        }
    }

    pub(super) fn write(self, content: &ClipContent) -> Result<(), PlatformError> {
        match self {
            Self::Wayland => write_wayland(content),
            Self::X11 => write_x11(content),
        }
    }

    /// Call `on_change` whenever the content differs from the last poll,
    /// until the guard drops.
    // shortcut: polling, switch to data-control events when wl-clipboard-rs exposes them
    pub(super) fn watch(self, on_change: Box<dyn Fn() + Send>) -> ClipboardWatch {
        let (stop, stopped) = mpsc::channel::<()>();
        let poller = thread::spawn(move || {
            let mut last = self.fingerprint();
            while let Err(RecvTimeoutError::Timeout) = stopped.recv_timeout(POLL) {
                let now = self.fingerprint();
                if now != last {
                    last = now;
                    on_change();
                }
            }
        });
        ClipboardWatch::new(move || {
            drop(stop);
            let _ = poller.join();
        })
    }

    /// What identifies the content without keeping it: kind and BLAKE3.
    fn fingerprint(self) -> Option<(ClipKind, blake3::Hash)> {
        let content = self.read().ok().flatten()?;
        Some((content.kind, blake3::hash(&content.bytes)))
    }
}

/// Whether the clipboard's `mimes` carry the password-manager hint;
/// `read_hint` yields the hint's content and runs only if it is offered.
pub(super) fn is_sensitive(mimes: &HashSet<String>, read_hint: impl FnOnce() -> Vec<u8>) -> bool {
    mimes.contains(HINT_MIME) && read_hint().trim_ascii() == SECRET
}

fn clipboard_err(e: impl std::fmt::Display) -> PlatformError {
    PlatformError::Clipboard(e.to_string())
}

fn read_wayland() -> Result<Option<ClipContent>, PlatformError> {
    let mimes = match paste::get_mime_types(ClipboardType::Regular, Seat::Unspecified) {
        Ok(mimes) => mimes,
        Err(paste::Error::ClipboardEmpty | paste::Error::NoSeats) => return Ok(None),
        Err(e) => return Err(clipboard_err(e)),
    };
    let (kind, wanted) = if mimes.iter().any(|m| m.starts_with("text/plain")) {
        (ClipKind::Text, paste::MimeType::Text)
    } else if mimes.contains(PNG_MIME) {
        (ClipKind::Png, paste::MimeType::Specific(PNG_MIME))
    } else {
        return Ok(None);
    };
    let bytes = wayland_bytes(wanted)?;
    let sensitive = is_sensitive(&mimes, || {
        wayland_bytes(paste::MimeType::Specific(HINT_MIME)).unwrap_or_default()
    });
    Ok(Some(ClipContent {
        kind,
        bytes,
        sensitive,
    }))
}

fn wayland_bytes(wanted: paste::MimeType<'_>) -> Result<Vec<u8>, PlatformError> {
    let (reader, _) = paste::get_contents(ClipboardType::Regular, Seat::Unspecified, wanted)
        .map_err(clipboard_err)?;
    let mut bytes = Vec::new();
    reader
        .take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(clipboard_err)?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err(PlatformError::Clipboard("content too large".into()));
    }
    Ok(bytes)
}

fn write_wayland(content: &ClipContent) -> Result<(), PlatformError> {
    let mime = match content.kind {
        ClipKind::Text => TEXT_MIME,
        ClipKind::Png => PNG_MIME,
    };
    Options::new()
        .copy(
            Source::Bytes(content.bytes.clone().into_boxed_slice()),
            copy::MimeType::Specific(mime.into()),
        )
        .map_err(clipboard_err)
}

// shortcut: X11 syncs text only (arboard's image data is raw RGBA), PNG needs an encoder.
fn read_x11() -> Result<Option<ClipContent>, PlatformError> {
    match arboard::Clipboard::new().map_err(clipboard_err)?.get_text() {
        Ok(text) => Ok(Some(ClipContent {
            kind: ClipKind::Text,
            bytes: text.into_bytes(),
            sensitive: false,
        })),
        Err(arboard::Error::ContentNotAvailable) => Ok(None),
        Err(e) => Err(clipboard_err(e)),
    }
}

fn write_x11(content: &ClipContent) -> Result<(), PlatformError> {
    if content.kind != ClipKind::Text {
        return Err(PlatformError::Unsupported(
            "clipboard_write of an image on X11",
        ));
    }
    let text = std::str::from_utf8(&content.bytes).map_err(clipboard_err)?;
    arboard::Clipboard::new()
        .map_err(clipboard_err)?
        .set_text(text)
        .map_err(clipboard_err)
}
