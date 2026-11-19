//! App update download as the user sees it: how far, how fast, how long,
//! and what an install waits for. Front-ends feed [`DownloadMeter`] the
//! updater's chunks and show the [`DownloadView`]s it lets through.

use std::time::{Duration, Instant};

use serde::Serialize;

/// Shortest gap between two download reports (at most 4 a second).
pub const REPORT_INTERVAL: Duration = Duration::from_millis(250);

/// How long the speed takes to follow a change: older readings fade with
/// this time constant, so bursts smooth out over a few seconds.
const SPEED_MEMORY: Duration = Duration::from_secs(2);

/// Below this many bytes a second no time left is given (it would be a
/// guess of hours that jumps about).
const MIN_SPEED: f64 = 1_000.0;

/// One look at a download in progress.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct DownloadView {
    /// Bytes downloaded so far.
    pub done: u64,
    /// Size of the package, if the server said.
    pub total: Option<u64>,
    /// Whole percent done; `None` while the size is unknown.
    pub percent: Option<u8>,
    /// Smoothed speed; `None` until there are two readings.
    pub bytes_per_sec: Option<u64>,
    /// Seconds left at that speed; `None` without a size or a usable speed.
    pub secs_left: Option<u64>,
}

/// Turns the updater's chunk callbacks into [`DownloadView`]s, at most one
/// per [`REPORT_INTERVAL`]. Time comes from the caller, so tests can run it
/// on a fake clock.
#[derive(Debug, Default)]
pub struct DownloadMeter {
    done: u64,
    /// Time and bytes done of the last report.
    last: Option<(Instant, u64)>,
    /// Bytes a second, smoothed (exponentially weighted moving average).
    speed: Option<f64>,
}

impl DownloadMeter {
    /// Counts `chunk` more bytes of a `total`-byte package, arrived at
    /// `now`. Returns a view when one is due: the first chunk, the last
    /// one, or [`REPORT_INTERVAL`] after the previous report.
    pub fn record(&mut self, chunk: u64, total: Option<u64>, now: Instant) -> Option<DownloadView> {
        self.done = self.done.saturating_add(chunk);
        let finished = total.is_some_and(|total| self.done >= total);
        if let Some((at, done)) = self.last {
            let elapsed = now.saturating_duration_since(at);
            if elapsed < REPORT_INTERVAL && !finished {
                return None;
            }
            self.measure(self.done - done, elapsed);
        }
        self.last = Some((now, self.done));
        Some(self.view(total))
    }

    /// Folds `bytes` over `elapsed` into the smoothed speed.
    fn measure(&mut self, bytes: u64, elapsed: Duration) {
        let secs = elapsed.as_secs_f64();
        if secs <= 0.0 {
            return;
        }
        let sample = bytes as f64 / secs;
        let weight = 1.0 - (-secs / SPEED_MEMORY.as_secs_f64()).exp();
        self.speed = Some(
            self.speed
                .map_or(sample, |speed| speed + weight * (sample - speed)),
        );
    }

    fn view(&self, total: Option<u64>) -> DownloadView {
        let remaining = total.map(|total| total.saturating_sub(self.done));
        DownloadView {
            done: self.done,
            total,
            percent: total.map(|total| percent(self.done, total)),
            bytes_per_sec: self.speed.map(|speed| speed.round() as u64),
            secs_left: remaining.and_then(|remaining| match self.speed {
                _ if remaining == 0 => Some(0),
                Some(speed) if speed >= MIN_SPEED => Some((remaining as f64 / speed).ceil() as u64),
                _ => None,
            }),
        }
    }
}

/// Whole percent of `done` out of `total`, at most 100.
fn percent(done: u64, total: u64) -> u8 {
    if total == 0 {
        return 100;
    }
    u8::try_from(done.saturating_mul(100) / total).map_or(100, |p| p.min(100))
}

/// What a downloaded update waits for before it installs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum UpdateBlocker {
    /// A move (send, receive, export or import) is running.
    Move,
    /// A linked phone's call is running on this computer.
    Call,
}

/// Why an update must not install now, if anything: installing can
/// close the app, which would stop a running move or call.
#[must_use]
pub fn update_blocker(moving: bool, on_call: bool) -> Option<UpdateBlocker> {
    if moving {
        Some(UpdateBlocker::Move)
    } else if on_call {
        Some(UpdateBlocker::Call)
    } else {
        None
    }
}
