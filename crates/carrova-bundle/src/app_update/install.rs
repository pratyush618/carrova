//! One approved update: download (unless done), check, wait for the
//! install permission, check again, hand to the OS installer.

use std::path::Path;
use std::time::Duration;

use carrova_platform::UpdateStage;

use super::download::{discard, remove, wished};
use super::{APK_MAX_SIZE, AppUpdateError, Control, Get, Job, Phone, Release, check_apk, download};

/// How often a held install looks again at the install permission.
const PERMISSION_RECHECK: Duration = Duration::from_secs(1);

/// Where an update is now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// A fetch starts (a new connection) at `from` of `total` bytes.
    Fetching {
        /// Bytes already on disk.
        from: u64,
        /// Size of the file, if known.
        total: Option<u64>,
    },
    /// `bytes` more arrived.
    Received {
        /// How many.
        bytes: u64,
    },
    /// The download stands at `done` of `total` bytes: paused, or waiting
    /// for Wi-Fi.
    Held {
        /// Why.
        stage: UpdateStage,
        /// Bytes on disk.
        done: u64,
        /// Size of the file, if known.
        total: Option<u64>,
    },
    /// Downloaded; checked against the release key and the installed build.
    Verifying,
    /// Checked; the OS needs the user's leave to install apps first.
    NeedsPermission,
    /// Handed to the OS installer, which asks the user.
    Installing,
}

/// Told each [`Step`] of an update.
pub trait Progress {
    /// The update reached `step`.
    fn step(&self, step: Step);
}

/// Downloads `release` into `dir` (unless already there), checks it, waits
/// for the install permission, checks it again and hands it to the OS
/// installer. `mobile_data`: Settings allow downloads on mobile data.
/// Other versions' downloads in `dir` are deleted first.
///
/// # Errors
///
/// [`AppUpdateError::Rejected`] if the APK is not a trusted, newer Carrova
/// (any failure to check it deletes it), the download's errors, or the OS
/// installer's.
pub fn install(
    get: &impl Get,
    phone: &(impl Phone + ?Sized),
    progress: &dyn Progress,
    control: &Control,
    release: &Release,
    dir: &Path,
    mobile_data: bool,
) -> Result<(), AppUpdateError> {
    let apk = dir.join(release.apk_name());
    let part = dir.join(format!("{}.part", release.apk_name()));
    clear_others(dir, &release.apk_name())?;
    if !apk.exists() {
        let job = Job {
            url: &release.apk_url(),
            part: &part,
            mobile_data,
            max_size: APK_MAX_SIZE,
        };
        download(get, phone, progress, control, job)?;
        std::fs::rename(&part, &apk)?;
        discard(&part);
    }
    progress.step(Step::Verifying);
    verify(phone, &apk)?;
    await_permission(phone, progress, control)?;
    // The wait can be long: the installed app may have changed meanwhile.
    verify(phone, &apk)?;
    progress.step(Step::Installing);
    Ok(phone.install(&apk)?)
}

/// Check `apk` against the installed app; delete it if that fails at all.
fn verify(phone: &(impl Phone + ?Sized), apk: &Path) -> Result<(), AppUpdateError> {
    let checked = (|| -> Result<(), AppUpdateError> {
        Ok(check_apk(&phone.inspect(apk)?, &phone.installed()?)?)
    })();
    if checked.is_err() {
        remove(apk);
    }
    checked
}

/// Returns once the app may install packages, or the user cancels.
fn await_permission(
    phone: &(impl Phone + ?Sized),
    progress: &dyn Progress,
    control: &Control,
) -> Result<(), AppUpdateError> {
    let mut asked = false;
    while !phone.can_install()? {
        wished(phone, control)?;
        if !asked {
            progress.step(Step::NeedsPermission);
            asked = true;
        }
        control.wait(PERMISSION_RECHECK);
    }
    Ok(())
}

/// Delete other versions' downloads from `dir` (files only; this
/// version's are named starting with `name`): one update at a time.
fn clear_others(dir: &Path, name: &str) -> Result<(), AppUpdateError> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let ours = entry.file_name().to_string_lossy().starts_with(name);
        if entry.file_type()?.is_file() && !ours {
            remove(&entry.path());
        }
    }
    Ok(())
}
