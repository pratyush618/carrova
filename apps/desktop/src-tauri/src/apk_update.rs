//! A phone's own app update: no store, so the APK comes from GitHub
//! releases. The same `CarrovaPort` update calls as the desktop's
//! [`updater`](crate::updater): `check_update` reads the release feed,
//! `install_update` (on the user's go-ahead) downloads the release's APK
//! ([`apk_download`]), checks it against the release key and the installed
//! build ([`check_apk`]), waits for Android's install permission if need
//! be, and hands it to Android's installer, which asks before it installs.
//! Steps arrive as [`BridgeEvent::Update`](carrova_view::ipc::BridgeEvent::Update).

mod http;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use carrova_core::time::now_unix;
use carrova_platform::{Platform, SemanticRoot};
use carrova_view::ipc::{BridgeEvent, ErrorCode, UpdateInfo, UpdateStatus, UpdateStep};
use carrova_view::update::apk::{
    AppVersion, FEED_MAX_SIZE, FEED_URL, Release, check_apk, just_updated,
};
use tauri::State;

use crate::apk_download::{self, Control, Job, Wish, cancelled};
use crate::command_error::CommandError;
use crate::events::Emit;
use crate::settings::SettingsStore;
use crate::state::AppState;
use crate::worker::blocking;
use http::Https;

/// The running app's version.
const RUNNING: &str = env!("CARGO_PKG_VERSION");
/// Folder of the cache root the APKs download to (shared with Android's
/// installer through the `FileProvider`'s `updates` path).
const UPDATES_DIR: &str = "updates";
/// How often a held install looks again at the install permission.
const PERMISSION_RECHECK: Duration = Duration::from_secs(1);

/// The release the user last saw offered, and their wishes for its
/// install in progress.
#[derive(Debug, Default)]
pub struct ApkUpdate {
    offered: Mutex<Option<Release>>,
    control: Arc<Control>,
}

impl ApkUpdate {
    /// The release on offer, if it is `version`.
    fn offered(&self, version: &str) -> Option<Release> {
        self.offered
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
            .filter(|release| release.version.to_string() == version)
    }
}

/// `CarrovaPort.checkUpdate`: asks the release feed whether a newer
/// version exists. Never downloads.
#[tauri::command]
pub async fn check_update(
    state: State<'_, AppState>,
    update: State<'_, ApkUpdate>,
) -> Result<UpdateStatus, CommandError> {
    let store = state.settings.clone();
    let release = blocking(move || {
        let release = Release::parse(&Https::default().get(FEED_URL, FEED_MAX_SIZE)?)
            .map_err(|e| CommandError::chained(ErrorCode::Network, &e))?;
        remember(&store, |s| s.last_checked = Some(now_unix()))?;
        Ok(release)
    })
    .await?;
    let running: AppVersion = RUNNING
        .parse()
        .map_err(|e| CommandError::chained(ErrorCode::Internal, &e))?;
    let newer = release.version > running;
    let status = if newer {
        UpdateStatus::Available(UpdateInfo {
            version: release.version.to_string(),
            current_version: RUNNING.to_owned(),
            notes: release.notes.clone(),
            date: release.date.clone(),
        })
    } else {
        UpdateStatus::UpToDate
    };
    *update
        .offered
        .lock()
        .unwrap_or_else(PoisonError::into_inner) = newer.then_some(release);
    Ok(status)
}

/// `CarrovaPort.installUpdate`: downloads `version` (the one the last
/// check offered; on mobile data only if Settings or `mobile_data` allow
/// it), checks it and hands it to Android's installer.
#[tauri::command]
pub async fn install_update(
    state: State<'_, AppState>,
    update: State<'_, ApkUpdate>,
    version: String,
    mobile_data: Option<bool>,
) -> Result<UpdateStatus, CommandError> {
    let release = update.offered(&version).ok_or_else(|| {
        CommandError::new(
            ErrorCode::Session,
            format!("version {version} is not on offer; check for updates again"),
        )
    })?;
    let control = Arc::clone(&update.control);
    control.update(|wish| {
        *wish = Wish {
            mobile_data: mobile_data.unwrap_or(false),
            ..Wish::default()
        };
    });
    let (platform, out, store) = (
        Arc::clone(&state.platform),
        Arc::clone(&state.out),
        state.settings.clone(),
    );
    blocking(move || {
        let allowed = store.load()?.app_update.mobile_data;
        install(platform.as_ref(), out.as_ref(), &control, &release, allowed)
    })
    .await?;
    Ok(UpdateStatus::Installed { version })
}

/// `CarrovaPort.cancelUpdate`: stops the update in progress; nothing is
/// installed.
#[tauri::command]
pub async fn cancel_update(update: State<'_, ApkUpdate>) -> Result<(), CommandError> {
    update.control.update(|wish| wish.cancelled = true);
    Ok(())
}

/// `CarrovaPort.installUpdateAnyway`: lets the download waiting for Wi-Fi
/// go on mobile data.
#[tauri::command]
pub async fn install_update_anyway(update: State<'_, ApkUpdate>) -> Result<(), CommandError> {
    update.control.update(|wish| wish.mobile_data = true);
    Ok(())
}

/// `CarrovaPort.pauseUpdate`: holds the download (`paused`) or carries on.
#[tauri::command]
pub async fn pause_update(update: State<'_, ApkUpdate>, paused: bool) -> Result<(), CommandError> {
    update.control.update(|wish| wish.paused = paused);
    Ok(())
}

/// `CarrovaPort.openInstallPermission`: Android's "Install unknown apps"
/// page for Carrova.
#[tauri::command]
pub async fn open_install_permission(state: State<'_, AppState>) -> Result<(), CommandError> {
    let platform = Arc::clone(&state.platform);
    blocking(move || Ok(platform.open_install_permission()?)).await
}

/// `CarrovaPort.takeUpdated`: the version the app was updated to since it
/// last ran, once (then the downloads are cleared); `None` otherwise.
#[tauri::command]
pub async fn take_updated(state: State<'_, AppState>) -> Result<Option<String>, CommandError> {
    let (platform, store) = (Arc::clone(&state.platform), state.settings.clone());
    blocking(move || {
        let last = store.load()?.app_update.last_run;
        if last.as_deref() == Some(RUNNING) {
            return Ok(None);
        }
        remember(&store, |s| s.last_run = Some(RUNNING.to_owned()))?;
        if !just_updated(last.as_deref(), RUNNING) {
            return Ok(None);
        }
        if let Ok(dir) = updates_dir(platform.as_ref())
            && let Err(error) = std::fs::remove_dir_all(dir)
        {
            tracing::debug!(%error, "no update downloads cleared");
        }
        Ok(Some(RUNNING.to_owned()))
    })
    .await
}

/// Download (unless already done), check, wait for the permission, and
/// hand `release` to the installer. `mobile_data`: Settings allow it.
fn install(
    platform: &dyn Platform,
    out: &dyn Emit,
    control: &Control,
    release: &Release,
    mobile_data: bool,
) -> Result<(), CommandError> {
    let dir = updates_dir(platform)?;
    let apk = dir.join(release.apk_name());
    let part = dir.join(format!("{}.part", release.apk_name()));
    clear_others(&dir, &[&apk, &part])?;
    if !apk.exists() {
        let job = Job {
            url: &release.apk_url(),
            part: &part,
            version: &release.version.to_string(),
            mobile_data,
        };
        apk_download::download(&Https::default(), platform, out, control, job)?;
        std::fs::rename(&part, &apk)?;
    }
    step(out, UpdateStep::Verifying);
    if let Err(rejected) = check_apk(
        &platform.inspect_package(&apk)?,
        &platform.installed_package()?,
    ) {
        std::fs::remove_file(&apk)?;
        return Err(CommandError::chained(ErrorCode::BadUpdate, &rejected));
    }
    await_permission(platform, out, control)?;
    step(out, UpdateStep::Installing);
    Ok(platform.install_package(&apk)?)
}

/// Returns once Carrova may install apps (the user turned it on in
/// Android's settings), or the user cancels.
fn await_permission(
    platform: &dyn Platform,
    out: &dyn Emit,
    control: &Control,
) -> Result<(), CommandError> {
    let mut asked = false;
    while !platform.can_install_packages()? {
        if control.wish().cancelled {
            return Err(cancelled());
        }
        if !asked {
            step(out, UpdateStep::NeedsPermission);
            asked = true;
        }
        std::thread::sleep(PERMISSION_RECHECK);
    }
    Ok(())
}

/// The folder APKs download to, created if missing.
fn updates_dir(platform: &dyn Platform) -> Result<PathBuf, CommandError> {
    let dir = platform
        .resolve_root(SemanticRoot::Cache)?
        .join(UPDATES_DIR);
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Delete other versions' downloads from `dir`: one update at a time.
fn clear_others(dir: &Path, keep: &[&Path]) -> Result<(), CommandError> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if !keep.contains(&path.as_path()) {
            std::fs::remove_file(&path)?;
        }
    }
    Ok(())
}

/// Change the app update bookkeeping in the saved settings.
fn remember(
    store: &SettingsStore,
    change: impl FnOnce(&mut carrova_view::ipc::AppUpdateSettings),
) -> Result<(), CommandError> {
    let _writing = store.lock();
    let mut settings = store.load()?;
    change(&mut settings.app_update);
    store.save(&settings)
}

fn step(out: &dyn Emit, step: UpdateStep) {
    out.emit(&BridgeEvent::Update { step });
}
