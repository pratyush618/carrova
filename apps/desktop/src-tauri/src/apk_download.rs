//! A phone's APK download (for `apk_update`): resumed from what is
//! already on disk (HTTP Range), paused and resumed by the user, held
//! while the phone is off Wi-Fi (unless mobile data is allowed), with
//! progress to the UI and the notification. [`Source`] and [`Phone`] are the seams tests drive.

use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::Path;
use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use carrova_platform::{Platform, UpdateNotice, UpdateStage, UpdateTap};
use carrova_view::ipc::{BridgeEvent, ErrorCode, UpdateStep};
use carrova_view::update::apk::download_blocker;
use carrova_view::update::{DownloadMeter, DownloadView, UpdateBlocker};

use crate::command_error::CommandError;
use crate::events::Emit;

/// How often a held download looks again (Wi-Fi, notification taps), and
/// a running one at the network and the taps.
const RECHECK: Duration = Duration::from_secs(1);
/// Bytes read at a time.
const CHUNK: usize = 64 * 1024;

/// What the user asked of the update in progress.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Wish {
    /// Hold the download where it is.
    pub paused: bool,
    /// Stop: nothing is installed.
    pub cancelled: bool,
    /// Download on mobile data too (for this update).
    pub mobile_data: bool,
}

/// Carries the user's wishes to the running update; a held update wakes
/// at once when one changes.
#[derive(Debug, Default)]
pub struct Control {
    wish: Mutex<Wish>,
    changed: Condvar,
}

impl Control {
    /// The user's wishes now.
    pub fn wish(&self) -> Wish {
        *self.lock()
    }

    /// Change the wishes with `change` and wake a held update.
    pub fn update(&self, change: impl FnOnce(&mut Wish)) {
        change(&mut self.lock());
        self.changed.notify_all();
    }

    /// Apply a tap on the notification's actions.
    fn tap(&self, tap: UpdateTap) {
        self.update(|wish| match tap {
            UpdateTap::Pause => wish.paused = true,
            UpdateTap::Resume => wish.paused = false,
            UpdateTap::Cancel => wish.cancelled = true,
        });
    }

    /// Wait up to `timeout` for a change.
    fn wait(&self, timeout: Duration) {
        let guard = self.lock();
        // Spurious wake-ups and timeouts alike just look again.
        drop(self.changed.wait_timeout(guard, timeout));
    }

    fn lock(&self) -> MutexGuard<'_, Wish> {
        self.wish.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// An HTTP body from some offset on.
pub struct Body {
    /// The bytes.
    pub reader: Box<dyn Read + Send>,
    /// Where they start: the offset asked for, or 0 if the server sent the
    /// whole file.
    pub start: u64,
    /// Size of the whole file, if the server said.
    pub total: Option<u64>,
}

impl std::fmt::Debug for Body {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Body")
            .field("start", &self.start)
            .field("total", &self.total)
            .finish_non_exhaustive()
    }
}

/// Where the APK comes from.
pub trait Source {
    /// The file at `url` from byte `offset` on.
    ///
    /// # Errors
    ///
    /// [`ErrorCode::Network`] if it cannot be fetched.
    fn open(&self, url: &str, offset: u64) -> Result<Body, CommandError>;
}

/// What the download needs of the phone.
pub trait Phone {
    /// Whether it is on Wi-Fi (or Ethernet) now.
    fn on_wifi(&self) -> bool;
    /// Show the download in the notification (`None`: remove it).
    fn show(&self, notice: Option<&UpdateNotice>);
    /// The user's last tap on the notification, taken once.
    fn tap(&self) -> Option<UpdateTap>;
}

impl<P: Platform + ?Sized> Phone for P {
    /// Unknown counts as off Wi-Fi: no download on mobile data by mistake.
    fn on_wifi(&self) -> bool {
        self.on_local_network().unwrap_or(false)
    }

    fn show(&self, notice: Option<&UpdateNotice>) {
        if let Err(error) = self.show_update(notice) {
            tracing::debug!(%error, "no update notification");
        }
    }

    fn tap(&self) -> Option<UpdateTap> {
        self.take_update_tap()
    }
}

/// One APK to download.
#[derive(Debug, Clone, Copy)]
pub struct Job<'a> {
    /// Where from.
    pub url: &'a str,
    /// The partial file it grows in (kept across pauses and failures).
    pub part: &'a Path,
    /// The version, for the notification.
    pub version: &'a str,
    /// The user allows mobile data for updates (Settings).
    pub mobile_data: bool,
}

/// How a fetch ended.
enum Fetched {
    /// The whole file is on disk.
    Whole,
    /// Paused, or off Wi-Fi, at this point of a file of `total` bytes:
    /// wait, then resume.
    Held { total: Option<u64> },
}

/// Downloads `job` into its partial file, which then holds the whole APK.
///
/// # Errors
///
/// [`ErrorCode::Cancelled`] if the user cancelled (the partial file is
/// deleted), else the network or disk failure (the partial file stays, so
/// trying again resumes).
pub fn download(
    source: &impl Source,
    phone: &(impl Phone + ?Sized),
    out: &dyn Emit,
    control: &Control,
    job: Job<'_>,
) -> Result<(), CommandError> {
    let mut total = None;
    let result = loop {
        if let Err(error) = hold(phone, out, control, job, total) {
            break Err(error);
        }
        match fetch(source, phone, out, control, job) {
            Ok(Fetched::Whole) => break Ok(()),
            Ok(Fetched::Held { total: size }) => total = size,
            // Wi-Fi dropped mid-read: wait for it rather than fail.
            Err(error)
                if error.bridge().code == ErrorCode::Network && off_wifi(phone, control, job) => {}
            Err(error) => break Err(error),
        }
    };
    if result
        .as_ref()
        .is_err_and(|e| e.bridge().code == ErrorCode::Cancelled)
    {
        remove(job.part);
    }
    phone.show(None);
    result
}

/// Returns once the download may run: not paused, and on Wi-Fi unless
/// mobile data is allowed. `total`: the file's size, if known.
fn hold(
    phone: &(impl Phone + ?Sized),
    out: &dyn Emit,
    control: &Control,
    job: Job<'_>,
    total: Option<u64>,
) -> Result<(), CommandError> {
    let mut shown = None;
    loop {
        let wish = wished(phone, control)?;
        let held = if wish.paused {
            Some(UpdateStage::Paused)
        } else {
            off_wifi(phone, control, job).then_some(UpdateStage::WaitingForWifi)
        };
        let Some(stage) = held else {
            return Ok(());
        };
        if shown != Some(stage) {
            let view = DownloadView::still(len(job.part), total);
            step(
                out,
                match stage {
                    UpdateStage::Paused => UpdateStep::Paused(view),
                    _ => UpdateStep::Waiting {
                        blocker: UpdateBlocker::Wifi,
                    },
                },
            );
            phone.show(Some(&notice(job, stage, view)));
            shown = Some(stage);
        }
        control.wait(RECHECK);
    }
}

/// Fetches the rest of the file, unless it is held before the end.
fn fetch(
    source: &impl Source,
    phone: &(impl Phone + ?Sized),
    out: &dyn Emit,
    control: &Control,
    job: Job<'_>,
) -> Result<Fetched, CommandError> {
    let body = source.open(job.url, len(job.part))?;
    let mut file = if body.start == 0 {
        File::create(job.part)?
    } else {
        OpenOptions::new().append(true).open(job.part)?
    };
    let mut meter = DownloadMeter::default();
    let report = |meter: &mut DownloadMeter, bytes: u64| {
        if let Some(view) = meter.record(bytes, body.total, Instant::now()) {
            step(out, UpdateStep::Downloading(view));
            phone.show(Some(&notice(job, UpdateStage::Downloading, view)));
        }
    };
    report(&mut meter, body.start);
    let (mut reader, mut done) = (body.reader, body.start);
    let mut buf = vec![0; CHUNK];
    let mut looked = Instant::now();
    loop {
        let n = reader.read(&mut buf).map_err(|e| network(&e))?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])?;
        done += n as u64;
        report(&mut meter, n as u64);
        if control.wish().cancelled {
            return Err(cancelled());
        }
        if looked.elapsed() >= RECHECK {
            looked = Instant::now();
            let wish = wished(phone, control)?;
            if wish.paused || off_wifi(phone, control, job) {
                file.sync_all()?;
                return Ok(Fetched::Held { total: body.total });
            }
        }
    }
    file.sync_all()?;
    match body.total {
        Some(total) if done < total => Err(network(&io::ErrorKind::UnexpectedEof.into())),
        Some(total) if done > total => {
            remove(job.part);
            Err(CommandError::new(
                ErrorCode::Network,
                "the download is bigger than the server said",
            ))
        }
        _ => Ok(Fetched::Whole),
    }
}

/// Whether the download may not run now for want of Wi-Fi.
fn off_wifi(phone: &(impl Phone + ?Sized), control: &Control, job: Job<'_>) -> bool {
    download_blocker(
        phone.on_wifi(),
        job.mobile_data || control.wish().mobile_data,
    )
    .is_some()
}

/// The user's wishes, after any notification tap; cancelled is an error.
fn wished(phone: &(impl Phone + ?Sized), control: &Control) -> Result<Wish, CommandError> {
    if let Some(tap) = phone.tap() {
        control.tap(tap);
    }
    let wish = control.wish();
    if wish.cancelled {
        return Err(cancelled());
    }
    Ok(wish)
}

fn notice(job: Job<'_>, stage: UpdateStage, view: DownloadView) -> UpdateNotice {
    UpdateNotice {
        version: job.version.to_owned(),
        stage,
        done: view.done,
        total: view.total,
        bytes_per_sec: view.bytes_per_sec,
    }
}

/// Bytes of `part` on disk (0 if there is none yet).
fn len(part: &Path) -> u64 {
    std::fs::metadata(part).map_or(0, |meta| meta.len())
}

fn remove(path: &Path) {
    if let Err(error) = std::fs::remove_file(path)
        && error.kind() != io::ErrorKind::NotFound
    {
        tracing::warn!(%error, "could not delete an update download");
    }
}

fn step(out: &dyn Emit, step: UpdateStep) {
    out.emit(&BridgeEvent::Update { step });
}

/// A failed read, kept apart from disk errors: trying again may work.
pub fn network(error: &io::Error) -> CommandError {
    CommandError::chained(ErrorCode::Network, error)
        .with_hint("Check your network connection and try again.")
}

/// The user cancelled the update.
pub fn cancelled() -> CommandError {
    CommandError::new(ErrorCode::Cancelled, "the update was cancelled")
}

#[cfg(test)]
#[path = "../tests/apk_download/unit.rs"]
mod tests;
