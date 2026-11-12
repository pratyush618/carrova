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
//! - [`recipes`], [`settings`], [`error_report`], [`updater`]: services.
//! - [`command_error`]: the one mapping of every failure to `BridgeError`.

pub mod command_error;
pub mod commands;
pub mod error_report;
pub mod events;
pub mod ops;
pub mod pairing;
pub mod recipes;
pub mod registry;
pub mod send_request;
pub mod settings;
pub mod state;
pub mod tray;
pub mod updater;
pub mod worker;

#[cfg(test)]
#[path = "../tests/support/mod.rs"]
mod testing;

use std::path::PathBuf;
use std::sync::Arc;

use error_report::Reports;
use state::{AppState, BridgeConfig};
use tauri::{AppHandle, Manager};
use updater::PendingUpdate;

/// Builds the Tauri application and runs it until the last window closes
/// with no tray icon up, or Quit.
///
/// Start-up installs the error-report panic hook (it records nothing
/// unless the user has allowed reports) and the bridge state, queues this
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
    let early_sends = pending_send.clone();
    tauri::Builder::default()
        // First, so a second launch is caught before anything else starts.
        .plugin(tauri_plugin_single_instance::init(move |app, argv, cwd| {
            send_request::on_second_instance(app, &early_sends, argv, &cwd);
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(move |app| {
            let platform: Arc<dyn carrova_platform::Platform> =
                Arc::from(carrova_platform::current());
            let reports = Reports::from_platform(Arc::clone(&platform))?;
            reports.install_panic_hook();
            app.manage(reports);
            let state = app_state(app.handle(), platform, pending_send)?;
            refresh_launchers(state.clone());
            send_request::forward(&state, &launch_send_paths());
            app.manage(state.clone());
            tray::start(app.handle(), &state, launched_in_background());
            Ok(())
        })
        .on_window_event(tray::on_window_event)
        .manage(PendingUpdate::default())
        .invoke_handler(tauri::generate_handler![
            commands::send::scan,
            commands::send::start_manual,
            commands::send::scan_apps,
            commands::send::pick,
            commands::send::take_send_request,
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
            updater::check_update,
            updater::install_update,
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
        .run(|app, event| tray::on_run_event(app, &event));
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

/// Whether this launch is a start at sign-in.
fn launched_in_background() -> bool {
    std::env::args_os().any(|arg| arg == tray::BACKGROUND_FLAG)
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
) -> Result<AppState, command_error::CommandError> {
    let resources = app
        .path()
        .resource_dir()
        .inspect_err(|e| tracing::warn!(error = %e, "no resource directory"))
        .ok();
    Ok(AppState {
        pending_send,
        ..AppState::new(
            platform,
            recipes::app_locations(resources.as_deref()),
            Arc::new(app.clone()),
            BridgeConfig::default(),
        )?
    })
}
