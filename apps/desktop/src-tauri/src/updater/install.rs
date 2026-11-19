//! One approved install: download (progress to the UI), signature check,
//! wait while a move or call runs, install. [`Package`] is the updater
//! seam, so tests run it without a release feed.

use std::time::{Duration, Instant};

use carrova_view::ipc::{BridgeEvent, ErrorCode, UpdateStep};
use carrova_view::update::{DownloadMeter, UpdateBlocker};
use tauri_plugin_updater::Update;
use tokio::sync::watch;

use crate::command_error::CommandError;
use crate::events::Emit;

/// How often a waiting install looks again at what holds it.
const RECHECK: Duration = Duration::from_millis(500);

/// What the user asked of the install in progress.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Signal {
    /// Carry on (each install starts with it).
    Go,
    /// Stop: nothing is installed.
    Cancel,
    /// Install now, even while a move or call runs.
    Anyway,
}

/// An update package as the install needs it.
pub trait Package {
    /// Downloads the package and checks its signature: `on_chunk(len,
    /// total)` per chunk, `on_downloaded` before the check.
    fn fetch(
        &self,
        on_chunk: impl FnMut(usize, Option<u64>) + Send,
        on_downloaded: impl FnOnce() + Send,
    ) -> impl Future<Output = Result<Vec<u8>, CommandError>> + Send;

    /// Installs the downloaded `bytes` (checked again first).
    fn install(&self, bytes: &[u8]) -> Result<(), CommandError>;
}

impl Package for Update {
    async fn fetch(
        &self,
        on_chunk: impl FnMut(usize, Option<u64>) + Send,
        on_downloaded: impl FnOnce() + Send,
    ) -> Result<Vec<u8>, CommandError> {
        Ok(self.download(on_chunk, on_downloaded).await?)
    }

    fn install(&self, bytes: &[u8]) -> Result<(), CommandError> {
        Ok(Self::install(self, bytes)?)
    }
}

/// Installs `package`, telling `out` each [`UpdateStep`]. While
/// `blocker` names something running, the verified package waits;
/// `signal` cancels (until installing starts) or installs anyway; a
/// closed `signal` never does.
///
/// # Errors
///
/// [`ErrorCode::Cancelled`] if cancelled, else the download, signature
/// or install failure. Nothing is installed in either case.
pub async fn install(
    package: &impl Package,
    out: &dyn Emit,
    blocker: impl Fn() -> Option<UpdateBlocker> + Send,
    mut signal: watch::Receiver<Signal>,
) -> Result<(), CommandError> {
    let bytes = tokio::select! {
        bytes = download(package, out) => bytes?,
        Ok(_) = signal.wait_for(|s| *s == Signal::Cancel) => return Err(cancelled()),
    };
    wait_until_free(out, blocker, &mut signal).await?;
    step(out, UpdateStep::Installing);
    package.install(&bytes)
}

async fn download(package: &impl Package, out: &dyn Emit) -> Result<Vec<u8>, CommandError> {
    let mut meter = DownloadMeter::default();
    let on_chunk = |len: usize, total: Option<u64>| {
        let chunk = u64::try_from(len).unwrap_or(u64::MAX);
        if let Some(view) = meter.record(chunk, total, Instant::now()) {
            step(out, UpdateStep::Downloading(view));
        }
    };
    package
        .fetch(on_chunk, || step(out, UpdateStep::Verifying))
        .await
}

/// Returns once nothing blocks the install, or the user says to install
/// anyway.
async fn wait_until_free(
    out: &dyn Emit,
    blocker: impl Fn() -> Option<UpdateBlocker>,
    signal: &mut watch::Receiver<Signal>,
) -> Result<(), CommandError> {
    let mut shown = None;
    while let Some(now) = blocker() {
        if shown != Some(now) {
            step(out, UpdateStep::Waiting { blocker: now });
            shown = Some(now);
        }
        tokio::select! {
            () = tokio::time::sleep(RECHECK) => {}
            Ok(seen) = signal.wait_for(|s| *s != Signal::Go) => {
                return if *seen == Signal::Anyway { Ok(()) } else { Err(cancelled()) };
            }
        }
    }
    Ok(())
}

fn step(out: &dyn Emit, step: UpdateStep) {
    out.emit(&BridgeEvent::Update { step });
}

fn cancelled() -> CommandError {
    CommandError::new(ErrorCode::Cancelled, "the update was cancelled")
}

#[cfg(test)]
#[path = "../../tests/updater/install/unit.rs"]
mod tests;
