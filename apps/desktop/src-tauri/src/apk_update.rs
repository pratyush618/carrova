//! A phone's own app update: the same `CarrovaPort` update calls as the
//! desktop's [`updater`](crate::updater), over
//! [`carrova_bundle::app_update`] (feed, resumable download, release-key
//! check, OS installer). Steps arrive as
//! [`BridgeEvent::Update`](carrova_view::ipc::BridgeEvent::Update), and
//! the download shows in a notification ([`PhoneSteps`]).

use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Instant;

use carrova_bundle::app_update::{
    self as update, Control, Https, Progress, Release, Step, just_updated,
};
use carrova_core::time::now_unix;
use carrova_platform::{Platform, SemanticRoot};
use carrova_view::ipc::{AppUpdateSettings, BridgeEvent, ErrorCode, UpdateInfo, UpdateStatus};
use carrova_view::update::{NoticeChange, PhoneSteps};
use tauri::State;

use crate::command_error::CommandError;
use crate::events::Emit;
use crate::settings::SettingsStore;
use crate::state::AppState;
use crate::worker::blocking;

/// The running app's version.
const RUNNING: &str = env!("CARGO_PKG_VERSION");
/// Folder of the cache root the APKs download to (shared with Android's
/// installer through the `FileProvider`'s `updates` path).
const UPDATES_DIR: &str = "updates";

/// The release the user last saw offered, and the update in progress.
#[derive(Debug, Default)]
pub struct ApkUpdate {
    offered: Mutex<Option<Release>>,
    control: Arc<Control>,
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
        let release = update::fetch_release(&Https::default())?;
        remember(&store, |s| s.last_checked = Some(now_unix()))?;
        Ok(release)
    })
    .await?;
    let newer = update::newer(release, RUNNING)
        .map_err(|e| CommandError::chained(ErrorCode::Internal, &e))?;
    let status = newer.as_ref().map_or(UpdateStatus::UpToDate, |release| {
        UpdateStatus::Available(UpdateInfo {
            version: release.version.to_string(),
            current_version: RUNNING.to_owned(),
            notes: release.notes.clone(),
            date: release.date.clone(),
        })
    });
    *update
        .offered
        .lock()
        .unwrap_or_else(PoisonError::into_inner) = newer;
    Ok(status)
}

/// `CarrovaPort.installUpdate`: downloads `version` (the one the last
/// check offered; on mobile data only if Settings or `mobile_data` allow
/// it), checks it and hands it to Android's installer, which asks.
#[tauri::command]
pub async fn install_update(
    state: State<'_, AppState>,
    update: State<'_, ApkUpdate>,
    version: String,
    mobile_data: Option<bool>,
) -> Result<UpdateStatus, CommandError> {
    let release = update
        .offered
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
        .filter(|release| release.version.to_string() == version)
        .ok_or_else(|| {
            CommandError::new(
                ErrorCode::Session,
                format!("version {version} is not on offer; check for updates again"),
            )
        })?;
    let (control, platform, out, store) = (
        Arc::clone(&update.control),
        Arc::clone(&state.platform),
        Arc::clone(&state.out),
        state.settings.clone(),
    );
    blocking(move || {
        let _running = control.begin(mobile_data.unwrap_or(false))?;
        let allowed = store.load()?.app_update.mobile_data;
        let dir = updates_dir(platform.as_ref())?;
        let shown = Shown::new(platform.as_ref(), out.as_ref(), &release);
        let result = update::install(
            &Https::default(),
            platform.as_ref(),
            &shown,
            &control,
            &release,
            &dir,
            allowed,
        );
        shown.change(&NoticeChange::Hide);
        Ok(result?)
    })
    .await?;
    Ok(UpdateStatus::Handed { version })
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

/// Each step to the UI and the notification.
struct Shown<'a> {
    platform: &'a dyn Platform,
    out: &'a dyn Emit,
    steps: Mutex<PhoneSteps>,
}

impl<'a> Shown<'a> {
    fn new(platform: &'a dyn Platform, out: &'a dyn Emit, release: &Release) -> Self {
        Self {
            platform,
            out,
            steps: Mutex::new(PhoneSteps::new(release.version.to_string())),
        }
    }

    fn change(&self, change: &NoticeChange) {
        let notice = match change {
            NoticeChange::Show(notice) => Some(notice),
            NoticeChange::Hide => None,
        };
        if let Err(error) = self.platform.show_update(notice) {
            tracing::debug!(%error, "no update notification");
        }
    }
}

impl Progress for Shown<'_> {
    fn step(&self, step: Step) {
        let (shown, notice) = self
            .steps
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .step(step, Instant::now());
        if let Some(step) = shown {
            self.out.emit(&BridgeEvent::Update { step });
        }
        if let Some(notice) = notice {
            self.change(&notice);
        }
    }
}

/// The folder APKs download to, created if missing.
fn updates_dir(platform: &dyn Platform) -> Result<PathBuf, CommandError> {
    let dir = platform
        .resolve_root(SemanticRoot::Cache)?
        .join(UPDATES_DIR);
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Change the app update bookkeeping in the saved settings.
fn remember(
    store: &SettingsStore,
    change: impl FnOnce(&mut AppUpdateSettings),
) -> Result<(), CommandError> {
    let _writing = store.lock();
    let mut settings = store.load()?;
    change(&mut settings.app_update);
    store.save(&settings)
}
