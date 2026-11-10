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

pub use sink::SessionSink;

/// Where bridge events go. Must not block (engine threads call it).
pub trait Emit: Send + Sync {
    /// Deliver `event` to the UI (best effort).
    fn emit(&self, event: &BridgeEvent);
}

impl Emit for AppHandle {
    fn emit(&self, event: &BridgeEvent) {
        // The event may hold a pairing code: log the failure, not the event.
        if let Err(error) = Emitter::emit(self, EVENT_CHANNEL, event) {
            tracing::warn!(%error, "could not deliver an event to the UI");
        }
    }
}
