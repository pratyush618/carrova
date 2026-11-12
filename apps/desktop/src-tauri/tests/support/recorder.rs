//! A recording [`Emit`] for tests.

use std::sync::Mutex;

use carrova_view::ipc::BridgeEvent;

use crate::events::Emit;

/// Records every event, as JSON (the shape the UI receives).
#[derive(Debug, Default)]
pub(crate) struct Recorder(Mutex<Vec<serde_json::Value>>);

impl Recorder {
    /// Everything emitted so far.
    pub(crate) fn events(&self) -> Vec<serde_json::Value> {
        self.0.lock().map(|v| v.clone()).unwrap_or_default()
    }

    /// The events of `kind` so far.
    pub(crate) fn of_kind(&self, kind: &str) -> Vec<serde_json::Value> {
        self.events()
            .into_iter()
            .filter(|e| e["kind"] == kind)
            .collect()
    }
}

impl Emit for Recorder {
    /// Recorded as a `popup` event with its `title` and `body`.
    fn popup(&self, title: &str, body: &str) {
        if let Ok(mut events) = self.0.lock() {
            events.push(serde_json::json!({ "kind": "popup", "title": title, "body": body }));
        }
    }

    fn emit(&self, event: &BridgeEvent) {
        let json = serde_json::to_value(event).unwrap_or_default();
        if let Ok(mut events) = self.0.lock() {
            events.push(json);
        }
    }
}
