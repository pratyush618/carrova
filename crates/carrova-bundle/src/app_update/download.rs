//! The APK download: resumed from the partial file when the server proves
//! it is the same file, paused and resumed by the user, held while the
//! phone is off Wi-Fi (unless mobile data is allowed), capped in size.

use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use carrova_platform::UpdateStage;

use super::http::{Get, Resume, open};
use super::{AppUpdateError, Control, NetFailure, Phone, Progress, Step, Wish, may_download};

/// How often a held download looks again (Wi-Fi, notification taps), and
/// a running one at the network and the taps.
const RECHECK: Duration = Duration::from_secs(1);
/// Bytes read at a time.
const CHUNK: usize = 64 * 1024;

/// One APK to download.
#[derive(Debug, Clone, Copy)]
pub struct Job<'a> {
    /// Where from.
    pub url: &'a str,
    /// The partial file it grows in (kept across pauses and failures),
    /// with the file's `ETag` beside it (`<part>.etag`).
    pub part: &'a Path,
    /// The user allows mobile data for updates (Settings).
    pub mobile_data: bool,
    /// Largest size accepted.
    pub max_size: u64,
}

/// How a fetch ended.
enum Fetched {
    /// The whole file is on disk.
    Whole,
    /// Paused, or off Wi-Fi, in a file of `total` bytes: wait, then resume.
    Held { total: Option<u64> },
}

/// Downloads `job` into its partial file, which then holds the whole APK.
///
/// # Errors
///
/// [`AppUpdateError::Cancelled`] if the user cancelled, or
/// [`AppUpdateError::TooLarge`] (the partial file is deleted either way),
/// else the network or disk failure (the partial file stays, so trying
/// again resumes).
pub fn download(
    get: &impl Get,
    phone: &(impl Phone + ?Sized),
    progress: &dyn Progress,
    control: &Control,
    job: Job<'_>,
) -> Result<(), AppUpdateError> {
    let mut total = None;
    let result = loop {
        if let Err(error) = hold(phone, progress, control, job, total) {
            break Err(error);
        }
        match fetch(get, phone, progress, control, job) {
            Ok(Fetched::Whole) => break Ok(()),
            Ok(Fetched::Held { total: size }) => total = size,
            // Wi-Fi dropped mid-read: wait for it rather than fail.
            Err(AppUpdateError::Network(_)) if off_wifi(phone, control, job) => {}
            Err(error) => break Err(error),
        }
    };
    if matches!(
        result,
        Err(AppUpdateError::Cancelled | AppUpdateError::TooLarge)
    ) {
        discard(job.part);
    }
    result
}

/// Delete a partial download and its `ETag`.
pub(super) fn discard(part: &Path) {
    remove(part);
    remove(&etag_path(part));
}

/// Returns once the download may run: not paused, and on Wi-Fi unless
/// mobile data is allowed. `total`: the file's size, if known.
fn hold(
    phone: &(impl Phone + ?Sized),
    progress: &dyn Progress,
    control: &Control,
    job: Job<'_>,
    total: Option<u64>,
) -> Result<(), AppUpdateError> {
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
            progress.step(Step::Held {
                stage,
                done: len(job.part),
                total,
            });
            shown = Some(stage);
        }
        control.wait(RECHECK);
    }
}

/// Fetches the rest of the file, unless it is held before the end.
fn fetch(
    get: &impl Get,
    phone: &(impl Phone + ?Sized),
    progress: &dyn Progress,
    control: &Control,
    job: Job<'_>,
) -> Result<Fetched, AppUpdateError> {
    let body = open(get, job.url, resume(job.part).as_ref())?;
    if body.total.is_some_and(|total| total > job.max_size) {
        return Err(AppUpdateError::TooLarge);
    }
    let mut file = if body.start == 0 {
        let file = File::create(job.part)?;
        match &body.etag {
            Some(etag) => std::fs::write(etag_path(job.part), etag)?,
            None => remove(&etag_path(job.part)),
        }
        file
    } else {
        OpenOptions::new().append(true).open(job.part)?
    };
    progress.step(Step::Fetching {
        from: body.start,
        total: body.total,
    });
    let (mut reader, mut done) = (body.reader, body.start);
    let mut buf = vec![0; CHUNK];
    let mut looked = Instant::now();
    loop {
        let n = reader
            .read(&mut buf)
            .map_err(|e| NetFailure::Read(e.kind()))?;
        if n == 0 {
            break;
        }
        done += n as u64;
        if done > job.max_size {
            return Err(AppUpdateError::TooLarge);
        }
        file.write_all(&buf[..n])?;
        progress.step(Step::Received { bytes: n as u64 });
        if control.wish().cancelled {
            return Err(AppUpdateError::Cancelled);
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
        Some(total) if done < total => Err(NetFailure::Short.into()),
        Some(total) if done > total => {
            discard(job.part);
            Err(NetFailure::Short.into())
        }
        _ => Ok(Fetched::Whole),
    }
}

/// Where the partial file stands, if it can be resumed (it has bytes and
/// the `ETag` they came with).
fn resume(part: &Path) -> Option<Resume> {
    let offset = len(part);
    let etag = std::fs::read_to_string(etag_path(part)).ok()?;
    (offset > 0).then_some(Resume { offset, etag })
}

/// Whether the download may not run now for want of Wi-Fi.
fn off_wifi(phone: &(impl Phone + ?Sized), control: &Control, job: Job<'_>) -> bool {
    !may_download(
        phone.on_wifi(),
        job.mobile_data || control.wish().mobile_data,
    )
}

/// The user's wishes, after any notification tap; cancelled is an error.
pub(super) fn wished(
    phone: &(impl Phone + ?Sized),
    control: &Control,
) -> Result<Wish, AppUpdateError> {
    if let Some(tap) = phone.tap() {
        control.tap(tap);
    }
    let wish = control.wish();
    if wish.cancelled {
        return Err(AppUpdateError::Cancelled);
    }
    Ok(wish)
}

fn etag_path(part: &Path) -> PathBuf {
    let mut name = part.as_os_str().to_owned();
    name.push(".etag");
    PathBuf::from(name)
}

/// Bytes of `part` on disk (0 if there is none yet).
fn len(part: &Path) -> u64 {
    std::fs::metadata(part).map_or(0, |meta| meta.len())
}

pub(super) fn remove(path: &Path) {
    if let Err(error) = std::fs::remove_file(path)
        && error.kind() != io::ErrorKind::NotFound
    {
        tracing::warn!(%error, "could not delete an update download");
    }
}
