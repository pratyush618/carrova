//! Progress throttling: the engine reports every few MiB, which on a fast
//! disk is far more often than a UI can show. Only `Progress` events are
//! ever dropped; state changes and everything else always pass.

use std::time::{Duration, Instant};

use carrova_core::{Event, Phase};

/// Shortest gap between two progress events of one session (≤ 20/s).
pub(crate) const PROGRESS_INTERVAL: Duration = Duration::from_millis(50);

/// Decides which progress events of one session reach the UI.
#[derive(Debug, Default)]
pub(crate) struct Throttle {
    /// Phase and time of the last progress event let through.
    last: Option<(Phase, Instant)>,
}

impl Throttle {
    /// Whether `event`, arriving at `now`, goes to the UI. A progress
    /// event passes when its phase changed, its phase is complete, or
    /// [`PROGRESS_INTERVAL`] passed since the last one.
    pub(crate) fn admit(&mut self, event: &Event, now: Instant) -> bool {
        let Event::Progress { phase, done, total } = *event else {
            return true;
        };
        let due = match self.last {
            Some((last_phase, at)) => {
                last_phase != phase || done >= total || now.duration_since(at) >= PROGRESS_INTERVAL
            }
            None => true,
        };
        if due {
            self.last = Some((phase, now));
        }
        due
    }
}

#[cfg(test)]
#[path = "../../tests/events/throttle/unit.rs"]
mod tests;
