//! Carrova desktop shell.
//!
//! A thin Tauri bridge between the React UI and the engine. No business
//! logic lives here (boundary rule 2): the CLI and the desktop app must
//! behave identically.
//!
//! # Architecture
//!
//! - [`commands`]: one Tauri command per `CarrovaPort` method, forwarding
//!   to [`ops`], the plain async operations over [`state::AppState`]
//!   (tests drive `ops` without a window).
//! - [`worker`]: every engine session object (`Exporter`, `Importer`,
//!   `SendHandle`) lives on its own thread and runs requests from a
//!   channel; commands await a oneshot reply. Blocking engine calls (and
//!   the transport's private runtime) never run on Tauri's async runtime.
//!   [`registry`] keeps each session's worker and its `Cancel`, outside
//!   the worker, so `cancel` reaches a running call.
//! - [`events`]: the engine's `EventSink` per session, forwarding to the
//!   UI as `BridgeEvent`s on `carrova://event`, progress throttled.
//! - [`pairing`]: the `SendPrompt` (codes as events, "new code?" answered
//!   by a command).
//! - [`send_request`]: `--send PATH…` from a file manager or a second
//!   launch (single-instance plugin), queued for the UI.
//! - [`tray`]: the window in background mode (closed to the tray,
//!   minimised or quit) and the tray menu.
//! - [`presence`]: this computer's advert on the LAN while the app runs.
//! - [`call_window`] (desktop): the always-on-top window of a linked
//!   phone's call, and the last call shown.
//! - [`ops::ecosystem`]: the link supervisor of linked devices, started
//!   with the app; its events go out as engine events of the link session.
//! - [`opened`]: links the app was opened with (a phone's share sheet,
//!   quick-settings tile or notification action), queued for the UI.
//! - `mobile` (phones only): one window, a process that outlives it, and
//!   the entry of the link service that keeps it alive.
//! - [`recipes`], [`settings`], [`error_report`], [`updater`] (desktop):
//!   services.
//! - [`command_error`]: the one mapping of every failure to `BridgeError`.

#[cfg(desktop)]
pub mod call_window;
pub mod command_error;
pub mod commands;
pub mod error_report;
pub mod events;
#[cfg(mobile)]
pub mod mobile;
pub mod opened;
pub mod ops;
pub mod pairing;
pub mod presence;
pub mod recipes;
pub mod registry;
pub mod send_request;
pub mod settings;
pub mod state;
#[cfg(desktop)]
pub mod tray;
#[cfg(desktop)]
pub mod updater;
pub mod worker;

#[cfg(test)]
#[path = "../tests/support/mod.rs"]
mod testing;

use std::path::PathBuf;
use std::sync::Arc;

use error_report::Reports;
use opened::Opened;
use presence::BackgroundPresence;
use state::{AppState, BridgeConfig};
use tauri::{AppHandle, Manager, RunEvent};

/// Builds the Tauri application and runs it until the last window closes
/// with no tray icon up, or Quit.
///
/// Start-up installs the error-report panic hook (it records nothing
/// unless the user has allowed reports) and the bridge state, starts the
/// presence advert (withdrawn on exit), queues this
/// launch's `--send` paths, refreshes the file-manager and autostart
/// entries in the background and opens the window (`--background`: the
/// tray instead, see [`tray`]). A second launch only hands its `--send` paths to this one
/// ([`send_request`]) and exits.
///
/// # Errors
///
/// Returns [`tauri::Error`] if the runtime or the main window fails to
/// start, or Carrova's state directory cannot be resolved.
pub fn run() -> tauri::Result<()> {
    // Before the builder: a second launch can arrive before `setup` runs.
    let pending_send = send_request::SendRequests::default();
    #[cfg(desktop)]
    let early_sends = pending_send.clone();
    let builder = tauri::Builder::default();
    // First, so a second launch is caught before anything else starts.
    #[cfg(desktop)]
    let builder = builder
        .plugin(tauri_plugin_single_instance::init(move |app, argv, cwd| {
            send_request::on_second_instance(app, &early_sends, argv, &cwd);
        }))
        .on_window_event(tray::on_window_event)
        // Phones update through their store or a new APK, not the updater.
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(updater::PendingUpdate::default());
    builder
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .setup(move |app| {
            let platform: Arc<dyn carrova_platform::Platform> =
                Arc::from(carrova_platform::current());
            let reports = Reports::from_platform(Arc::clone(&platform))?;
            reports.install_panic_hook();
            app.manage(reports);
            let presence = Arc::new(BackgroundPresence::new(Arc::clone(&platform)));
            let state = app_state(app.handle(), platform, pending_send, presence.clone())?;
            refresh_launchers(state.clone());
            start_presence(&presence);
            start_link(state.clone());
            app.manage(presence);
            send_request::forward(&state, &launch_send_paths());
            app.manage(state.clone());
            #[cfg(desktop)]
            tray::start(app.handle(), &state, launched_in_background());
            #[cfg(desktop)]
            call_window::start(app.handle());
            #[cfg(mobile)]
            mobile::open_main_window(app.handle())?;
            Ok(())
        })
        .manage(Opened::default())
        .invoke_handler(tauri::generate_handler![
            commands::send::scan,
            commands::send::start_manual,
            commands::send::scan_apps,
            commands::send::pick,
            commands::send::take_send_request,
            commands::send::take_opened,
            #[cfg(mobile)]
            mobile::scan_qr,
            commands::send::suggest,
            commands::send::select,
            commands::send::passphrase_strength,
            commands::send::export_archive,
            commands::send::run_send,
            commands::send::answer_pairing,
            commands::receive::open_archive,
            commands::receive::connect,
            commands::receive::plan,
            commands::receive::edit_plan,
            commands::receive::plan_options,
            commands::receive::approve,
            commands::receive::apply,
            commands::receive::end_review,
            commands::receive::set_transfer_paused,
            commands::receive::ignore_free_space,
            commands::receive::retry_connection,
            commands::receive::browse_nearby,
            commands::sessions::cancel,
            commands::sessions::cancel_connect,
            commands::sessions::resume,
            commands::sessions::sessions,
            commands::sessions::rollback,
            commands::sessions::rollback_preview,
            commands::sessions::about,
            commands::sessions::dismiss,
            commands::recipes::catalog,
            commands::recipes::recipes_status,
            commands::recipes::recipes_update,
            commands::recipes::recipes_check,
            commands::recipes::recipes_reset,
            commands::inbox::notices,
            commands::inbox::mark_notices_read,
            commands::settings::settings,
            commands::settings::save_settings,
            commands::integration::file_manager_status,
            commands::integration::set_file_manager_entries,
            commands::background::set_background,
            commands::devices::nearby,
            commands::devices::known_devices,
            commands::devices::rename_device,
            commands::devices::forget_device,
            commands::devices::reset_identity,
            commands::devices::visibility,
            commands::devices::set_visibility,
            commands::devices::wake_device,
            commands::devices::take_show_nearby,
            commands::ecosystem::linked_devices,
            commands::ecosystem::pair_invite,
            commands::ecosystem::link_computer,
            commands::ecosystem::link_invite,
            commands::ecosystem::answer_pair_peer,
            commands::ecosystem::own_fingerprint,
            commands::ecosystem::lan_available,
            commands::ecosystem::set_feature,
            commands::ecosystem::answer_ask,
            commands::ecosystem::send_clipboard,
            commands::ecosystem::send_text,
            commands::ecosystem::send_files,
            commands::ecosystem::handoff_url,
            commands::ecosystem::ring,
            commands::ecosystem::stop_ring,
            commands::ecosystem::clip_history,
            commands::ecosystem::clear_clip_history,
            commands::ecosystem::clip_settings,
            commands::ecosystem::set_clip_settings,
            commands::ecosystem::retry_file,
            commands::phone::phone_notes,
            commands::phone::phone_note_action,
            commands::phone::phone_apps,
            commands::phone::remind_phone,
            commands::phone::phone_threads,
            commands::phone::phone_thread,
            commands::phone::send_sms,
            commands::phone::phone_sims,
            commands::phone::phone_call,
            commands::phone::call_history,
            commands::phone::call_window,
            #[cfg(desktop)]
            call_window::call_now,
            commands::phone::phone_settings,
            commands::phone::set_phone_settings,
            commands::phone::call_audio_step,
            commands::phone::install_call_audio,
            commands::phone::set_call_audio_config,
            commands::phone::phone_perm_step,
            commands::phone::request_phone_perm,
            commands::phone::open_perm_settings,
            commands::phone::notify_apps,
            commands::phone::set_notify_app,
            commands::phone::set_notify_filters,
            commands::phone::messages_sync,
            commands::phone::set_messages_sync,
            #[cfg(desktop)]
            updater::check_update,
            #[cfg(desktop)]
            updater::install_update,
            #[cfg(desktop)]
            updater::restart_app,
            commands::reports::error_report_consent,
            commands::reports::set_error_report_consent,
            commands::reports::error_reports,
            commands::reports::error_report,
            commands::reports::delete_error_report,
            commands::reports::clear_error_reports,
            commands::reports::error_report_issue,
        ])
        .build(tauri::generate_context!())?
        .run(|app, event| {
            if let RunEvent::Exit = event
                && let Some(presence) = app.try_state::<Arc<BackgroundPresence>>()
            {
                presence.stop();
            }
            #[cfg(desktop)]
            tray::on_run_event(app, &event);
            #[cfg(mobile)]
            mobile::on_run_event(app, &event);
        });
    Ok(())
}

/// The `--send` paths this launch was started with, if any.
fn launch_send_paths() -> Vec<PathBuf> {
    let argv: Vec<_> = std::env::args_os().collect();
    let cwd = std::env::current_dir()
        .inspect_err(|e| tracing::warn!(error = %e, "no working directory"))
        .unwrap_or_default();
    send_request::send_paths(&argv, &cwd).unwrap_or_default()
}

/// Entry point the Android and iOS runtimes call (they cannot take a
/// `Result`). The macro emits `extern` JNI glue, hence the one `unsafe`
/// exception to the workspace's `forbid` (ADR 0003 sign-off).
#[cfg(mobile)]
#[allow(unsafe_code)]
#[tauri::mobile_entry_point]
fn mobile_main() {
    if let Err(error) = run() {
        tracing::error!(%error, "the app failed to run");
    }
}

/// Whether this launch is a start at sign-in.
#[cfg(desktop)]
fn launched_in_background() -> bool {
    std::env::args_os().any(|arg| arg == tray::BACKGROUND_FLAG)
}

/// Start advertising this computer, off the start-up path.
fn start_presence(presence: &Arc<BackgroundPresence>) {
    let presence = Arc::clone(presence);
    tauri::async_runtime::spawn_blocking(move || presence.start());
}

/// Start the link supervisor (or attach to another process's), off the
/// start-up path; on a phone the link service may have started it already
/// ([`ops::ecosystem::start_once`]).
fn start_link(state: AppState) {
    tauri::async_runtime::spawn_blocking(move || {
        if let Err(error) = ops::ecosystem::start_link(&state) {
            tracing::warn!(%error, "linked devices are unavailable");
        }
    });
}

/// Point the file-manager and autostart entries at this app's launcher,
/// off the start-up path.
fn refresh_launchers(state: AppState) {
    tauri::async_runtime::spawn_blocking(move || {
        ops::integration::refresh_file_manager_entries(&state);
        ops::background::refresh_autostart(&state);
    });
}

/// The bridge state of the running app: recipes from the app's resources
/// (see [`recipes::app_locations`]), events to the app's windows, sent
/// paths queued in `pending_send`.
fn app_state(
    app: &AppHandle,
    platform: Arc<dyn carrova_platform::Platform>,
    pending_send: send_request::SendRequests,
    presence: Arc<BackgroundPresence>,
) -> Result<AppState, command_error::CommandError> {
    let resources = app
        .path()
        .resource_dir()
        .inspect_err(|e| tracing::warn!(error = %e, "no resource directory"))
        .ok();
    Ok(AppState {
        pending_send,
        link: ops::ecosystem::process_link(),
        ..AppState::new(
            platform,
            recipes::app_locations(resources.as_deref()),
            Arc::new(app.clone()),
            presence,
            BridgeConfig::default(),
        )?
    })
}
