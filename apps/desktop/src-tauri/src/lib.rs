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
pub mod settings;
pub mod state;
pub mod updater;
pub mod worker;

#[cfg(test)]
#[path = "../tests/support/mod.rs"]
mod testing;

use std::sync::Arc;

use error_report::Reports;
use state::{AppState, BridgeConfig};
use tauri::{AppHandle, Manager};
use updater::PendingUpdate;

/// Builds the Tauri application and runs it until the last window closes.
///
/// Start-up installs the error-report panic hook (it records nothing
/// unless the user has allowed reports) and the bridge state.
///
/// # Errors
///
/// Returns [`tauri::Error`] if the runtime or the main window fails to
/// start, or Carrova's state directory cannot be resolved.
pub fn run() -> tauri::Result<()> {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let platform: Arc<dyn carrova_platform::Platform> =
                Arc::from(carrova_platform::current());
            let reports = Reports::from_platform(Arc::clone(&platform))?;
            reports.install_panic_hook();
            app.manage(reports);
            app.manage(app_state(app.handle(), platform));
            Ok(())
        })
        .manage(PendingUpdate::default())
        .invoke_handler(tauri::generate_handler![
            commands::send::scan,
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
            commands::settings::settings,
            commands::settings::save_settings,
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
        .run(tauri::generate_context!())
}

/// The bridge state of the running app: recipes from the app's resources
/// (see [`recipes::app_locations`]), events to the app's windows.
fn app_state(app: &AppHandle, platform: Arc<dyn carrova_platform::Platform>) -> AppState {
    let resources = app
        .path()
        .resource_dir()
        .inspect_err(|e| tracing::warn!(error = %e, "no resource directory"))
        .ok();
    AppState::new(
        platform,
        recipes::app_locations(resources.as_deref()),
        Arc::new(app.clone()),
        BridgeConfig::default(),
    )
}
