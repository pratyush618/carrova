//! The clipboard over links: what this side sends (on each change while
//! watched, or on request) and the received history. Contents are never
//! logged.

use std::sync::mpsc;
use std::sync::{Arc, MutexGuard, PoisonError};
use std::thread::JoinHandle;

use carrova_crypto::identity::DeviceId;
use carrova_link::{
    ClipHistory, ClipItem, Feature, FeatureState, ReceivedClip, allowed, send_clip,
};
use carrova_platform::{ClipContent, ClipKind, ClipboardWatch, PlatformError};
use carrova_transport::Connection;
use serde::{Deserialize, Serialize};

use super::supervisor::{Shared, spawn};
use crate::error::Result;
use crate::session::Event;

/// Why the clipboard was not shared ([`Event::ClipSkipped`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum ClipSkip {
    /// The OS marked it sensitive (a password manager's copy).
    Sensitive,
    /// Over the size limit for its kind.
    TooBig,
}

/// Clipboard state: received history, and the content last sent or
/// written here (a change to it is not sent back).
#[derive(Debug, Default)]
pub(super) struct Clips {
    history: ClipHistory,
    last: Option<ClipContent>,
}

impl Clips {
    pub(super) fn history(&self) -> Vec<ReceivedClip> {
        self.history.items().cloned().collect()
    }

    /// Note a received clip, before it is written here (so the watch
    /// does not send it back).
    pub(super) fn received(&mut self, content: ClipContent, item: ClipItem) {
        self.last = Some(content);
        self.history.push(item, crate::time::now_unix());
    }

    pub(super) fn clear(&mut self) {
        self.history.clear();
    }
}

pub(super) fn lock_clips(shared: &Shared) -> MutexGuard<'_, Clips> {
    shared.clips.lock().unwrap_or_else(PoisonError::into_inner)
}

/// `content` as a clip to send; `None` ([`Event::ClipSkipped`]) if it is
/// sensitive or too big.
pub(super) fn clip_item(shared: &Shared, content: ClipContent) -> Option<ClipItem> {
    let skip = |reason| {
        shared.emit(Event::ClipSkipped { reason });
        None
    };
    if content.sensitive {
        return skip(ClipSkip::Sensitive);
    }
    let item = match content.kind {
        ClipKind::Text => ClipItem::Text(String::from_utf8(content.bytes).ok()?),
        ClipKind::Png => ClipItem::Png(content.bytes),
    };
    match item.check() {
        Ok(()) => Some(item),
        Err(_) => skip(ClipSkip::TooBig),
    }
}

/// Send `item` to `device` over `conn`.
pub(super) fn send_clip_to(
    shared: &Shared,
    device: &DeviceId,
    conn: &Connection,
    item: &ClipItem,
) -> Result<()> {
    send_clip(conn, item)?;
    shared.emit(Event::ClipSent { device: *device });
    Ok(())
}

/// Watch the clipboard and send each change to every linked device that
/// takes it. `None` where the platform cannot watch (Android sends by
/// user action only). The thread ends when the watch drops.
pub(super) fn watch_clipboard(
    shared: &Arc<Shared>,
) -> Result<Option<(ClipboardWatch, JoinHandle<()>)>> {
    let (tx, rx) = mpsc::channel();
    let changed = Box::new(move || {
        let _ = tx.send(());
    });
    let watch = match shared.platform().watch_clipboard(changed) {
        Ok(watch) => watch,
        Err(PlatformError::Unsupported(_)) => return Ok(None),
        Err(e) => {
            tracing::warn!(error = %e, "clipboard not watched");
            return Ok(None);
        }
    };
    let shared = Arc::clone(shared);
    let thread = spawn("carrova-link-clipboard", move || {
        while rx.recv().is_ok() {
            while rx.try_recv().is_ok() {}
            clipboard_changed(&shared);
        }
    })?;
    Ok(Some((watch, thread)))
}

fn clipboard_changed(shared: &Shared) {
    let content = match shared.platform().clipboard_read() {
        Ok(Some(content)) => content,
        Ok(None) => return,
        Err(e) => return tracing::debug!(error = %e, "clipboard unreadable"),
    };
    {
        let mut clips = lock_clips(shared);
        if clips.last.as_ref() == Some(&content) {
            return;
        }
        clips.last = Some(content.clone());
    }
    let targets: Vec<_> = shared
        .peers()
        .iter()
        .filter_map(|p| {
            let link = p.link.as_ref()?;
            let on = allowed(&p.local, link.remote.as_ref()?, Feature::Clipboard);
            (on == FeatureState::On).then(|| (p.id(), Arc::clone(&link.conn)))
        })
        .collect();
    if targets.is_empty() {
        return;
    }
    let Some(item) = clip_item(shared, content) else {
        return;
    };
    for (device, conn) in targets {
        if let Err(e) = send_clip_to(shared, &device, &conn, &item) {
            tracing::debug!(error = %e, "clipboard not sent");
        }
    }
}
