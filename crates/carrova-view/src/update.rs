//! App update download as the user sees it: how far, how fast, how long,
//! and what an install waits for. Front-ends feed [`DownloadMeter`] the
//! updater's chunks and show the [`DownloadView`]s it lets through. A
//! sideloaded phone's update steps become UI steps and notification
//! content through [`PhoneSteps`]; its rules are in [`apk`].

pub mod apk;

use std::time::{Duration, Instant};

use carrova_bundle::app_update::Step;
use carrova_platform::{UpdateNotice, UpdateStage};
use serde::Serialize;

use crate::ipc::UpdateStep;

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

impl DownloadView {
    /// A download standing still at `done` of `total` bytes (paused, or
    /// waiting): no speed, no time left.
    #[must_use]
    pub fn still(done: u64, total: Option<u64>) -> Self {
        Self {
            done,
            total,
            percent: total.map(|total| percent(done, total)),
            bytes_per_sec: None,
            secs_left: None,
        }
    }
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
            bytes_per_sec: self.speed.map(|speed| speed.round() as u64),
            secs_left: remaining.and_then(|remaining| match self.speed {
                _ if remaining == 0 => Some(0),
                Some(speed) if speed >= MIN_SPEED => Some((remaining as f64 / speed).ceil() as u64),
                _ => None,
            }),
            ..DownloadView::still(self.done, total)
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

/// What holds an update: an install waits while a move or call runs, a
/// phone's download while it is off Wi-Fi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum UpdateBlocker {
    /// A move (send, receive, export or import) is running.
    Move,
    /// A linked phone's call is running on this computer.
    Call,
    /// A phone is off Wi-Fi and may not download on mobile data.
    Wifi,
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

/// What the phone's update notification does next.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoticeChange {
    /// Show (or update) it.
    Show(UpdateNotice),
    /// Remove it: the download is over.
    Hide,
}

/// Turns a phone update's [`Step`]s into the [`UpdateStep`]s the UI shows
/// (download progress at most every [`REPORT_INTERVAL`]) and the changes
/// of its notification.
#[derive(Debug)]
pub struct PhoneSteps {
    version: String,
    meter: DownloadMeter,
    total: Option<u64>,
}

impl PhoneSteps {
    /// The steps of the update to `version`.
    #[must_use]
    pub fn new(version: String) -> Self {
        Self {
            version,
            meter: DownloadMeter::default(),
            total: None,
        }
    }

    /// What `step`, at `now`, shows: a UI step and a notification change,
    /// each if due.
    pub fn step(&mut self, step: Step, now: Instant) -> (Option<UpdateStep>, Option<NoticeChange>) {
        match step {
            Step::Fetching { from, total } => {
                (self.meter, self.total) = (DownloadMeter::default(), total);
                self.downloading(from, now)
            }
            Step::Received { bytes } => self.downloading(bytes, now),
            Step::Held { stage, done, total } => {
                let view = DownloadView::still(done, total.or(self.total));
                let shown = match stage {
                    UpdateStage::Paused => UpdateStep::Paused(view),
                    _ => UpdateStep::Waiting {
                        blocker: UpdateBlocker::Wifi,
                    },
                };
                (Some(shown), Some(self.notice(stage, view)))
            }
            Step::Verifying => (Some(UpdateStep::Verifying), Some(NoticeChange::Hide)),
            Step::NeedsPermission => (Some(UpdateStep::NeedsPermission), None),
            Step::Installing => (Some(UpdateStep::Installing), None),
        }
    }

    fn downloading(
        &mut self,
        bytes: u64,
        now: Instant,
    ) -> (Option<UpdateStep>, Option<NoticeChange>) {
        match self.meter.record(bytes, self.total, now) {
            Some(view) => (
                Some(UpdateStep::Downloading(view)),
                Some(self.notice(UpdateStage::Downloading, view)),
            ),
            None => (None, None),
        }
    }

    fn notice(&self, stage: UpdateStage, view: DownloadView) -> NoticeChange {
        NoticeChange::Show(UpdateNotice {
            version: self.version.clone(),
            stage,
            done: view.done,
            total: view.total,
            bytes_per_sec: view.bytes_per_sec,
        })
    }
}
