//! Android adapter.
//!
//! Filesystem primitives and `PATH` lookup are the Unix ones shared with
//! Linux (`crate::sys`). Clipboard, Downloads, opening, ring and the
//! multicast lock go through one static Kotlin method each
//! (`org.byteveda.carrova.platform.*`), reached over JNI in [`jni`] (ADR
//! 0003). The helpers here are pure, so they are tested on any host.

use crate::ClipKind;

#[cfg(target_os = "android")]
mod clipboard;
#[cfg(target_os = "android")]
mod files;
#[cfg(target_os = "android")]
mod jni;
#[cfg(target_os = "android")]
mod net;
#[cfg(target_os = "android")]
mod platform;
#[cfg(target_os = "android")]
mod ring;
#[cfg(test)]
#[path = "../../tests/android/unit.rs"]
mod tests;

#[cfg(target_os = "android")]
pub use jni::attach_service_context;
#[cfg(target_os = "android")]
pub(crate) use platform::AndroidPlatform;

/// What Carrova syncs of a clip offered as `mimes`: text (any `text/*`,
/// read as text) before PNG; `None` for anything else.
fn clip_kind<'a>(mimes: impl IntoIterator<Item = &'a str>) -> Option<ClipKind> {
    let mut kind = None;
    for mime in mimes {
        if mime.starts_with("text/") {
            return Some(ClipKind::Text);
        }
        if mime == "image/png" {
            kind = Some(ClipKind::Png);
        }
    }
    kind
}

/// Whether a clip is secret, from its `ClipDescription.EXTRA_IS_SENSITIVE`
/// extra as text (`None`: not set). Anything but `"false"` counts as set:
/// fail closed.
fn is_sensitive(extra: Option<&str>) -> bool {
    extra.is_some_and(|value| value != "false")
}

/// Whether `len` clip bytes are within the cap Linux applies too. The glue
/// reads at most one byte past it, so an endless stream is refused here.
fn within_cap(len: usize) -> bool {
    len as u64 <= crate::clipboard::MAX_BYTES
}
