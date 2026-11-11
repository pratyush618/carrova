//! Engine → UI events: every notification reaches the UI as one
//! [`BridgeEvent`] on [`EVENT_CHANNEL`].
//!
//! [`Emit`] is the outlet (the Tauri app handle in the app, a recorder in
//! tests); [`SessionSink`] is the engine's [`EventSink`](carrova_core::EventSink) for one session:
//! it tags events with the session and throttles progress.

mod sink;
mod throttle;

use carrova_view::ipc::{BridgeEvent, EVENT_CHANNEL};
use tauri::{AppHandle, Emitter};
use tauri_plugin_notification::NotificationExt;

pub use sink::SessionSink;

/// Where bridge events go, and system pop-ups. Must not block (engine
/// threads call it).
pub trait Emit: Send + Sync {
    /// Show a system pop-up (best effort).
    fn popup(&self, title: &str, body: &str) {
        let _ = (title, body);
    }

    /// Deliver `event` to the UI (best effort).
    fn emit(&self, event: &BridgeEvent);
}

impl Emit for AppHandle {
    fn popup(&self, title: &str, body: &str) {
        if let Err(error) = self.notification().builder().title(title).body(body).show() {
            tracing::debug!(%error, "could not show a pop-up");
        }
    }

    fn emit(&self, event: &BridgeEvent) {
        // The event may hold a pairing code: log the failure, not the event.
        if let Err(error) = Emitter::emit(self, EVENT_CHANNEL, event) {
            tracing::warn!(%error, "could not deliver an event to the UI");
        }
    }
}
