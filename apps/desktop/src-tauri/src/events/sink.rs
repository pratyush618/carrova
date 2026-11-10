//! The engine's event sink for one session.

use std::sync::{Arc, Mutex, PoisonError};
use std::time::Instant;

use carrova_core::{Event, EventSink, SessionId};
use carrova_view::ipc::BridgeEvent;

use super::Emit;
use super::throttle::Throttle;

/// Events kept while the session id is not known yet (a session object
/// can report before its constructor returns the id).
const MAX_PENDING: usize = 256;

/// Forwards one session's engine events as [`BridgeEvent::Engine`],
/// throttling progress (at most one progress event per 50 ms).
///
/// The sink exists before its session: it is created with the engine that
/// opens the session. Until [`Self::bind`] (or the session's first
/// `StateChanged`, which names it) events are held, then delivered in
/// order.
pub struct SessionSink {
    out: Arc<dyn Emit>,
    inner: Mutex<Inner>,
}

#[derive(Default)]
struct Inner {
    session: Option<SessionId>,
    pending: Vec<Event>,
    throttle: Throttle,
}

impl std::fmt::Debug for SessionSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionSink").finish_non_exhaustive()
    }
}

impl SessionSink {
    /// A sink for a session that is not known yet.
    pub fn unbound(out: Arc<dyn Emit>) -> Self {
        Self {
            out,
            inner: Mutex::new(Inner::default()),
        }
    }

    /// A sink for the existing session `session`.
    pub fn bound(out: Arc<dyn Emit>, session: &SessionId) -> Self {
        let sink = Self::unbound(out);
        sink.bind(session);
        sink
    }

    /// Name the session and deliver the events held so far. Later calls
    /// are ignored (a sink serves one session).
    pub fn bind(&self, session: &SessionId) {
        let mut inner = self.lock();
        for event in inner.bind(session) {
            self.forward(session, event);
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn forward(&self, session: &SessionId, event: Event) {
        self.out.emit(&BridgeEvent::Engine {
            session: session.clone(),
            event,
        });
    }
}

impl Inner {
    /// Record `session` if none is set yet; returns the held events to
    /// deliver now (none if the sink was already bound).
    fn bind(&mut self, session: &SessionId) -> Vec<Event> {
        if self.session.is_some() {
            return Vec::new();
        }
        self.session = Some(session.clone());
        std::mem::take(&mut self.pending)
    }
}

impl EventSink for SessionSink {
    fn emit(&self, event: Event) {
        let mut inner = self.lock();
        if !inner.throttle.admit(&event, Instant::now()) {
            return;
        }
        if let Event::StateChanged { session, .. } = &event {
            for held in inner.bind(session) {
                self.forward(session, held);
            }
        }
        match &inner.session {
            Some(session) => self.forward(session, event),
            None if inner.pending.len() < MAX_PENDING => inner.pending.push(event),
            None => {}
        }
    }
}

#[cfg(test)]
#[path = "../../tests/events/sink/unit.rs"]
mod tests;
