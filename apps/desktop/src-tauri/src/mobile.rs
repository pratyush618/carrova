//! The app on a phone: no tray, one window, and a process that outlives
//! it. Android's `LinkService` (a foreground service) keeps the process,
//! and the link supervisor in it, running after the user closes the app,
//! and starts them after boot with no app at all (ADR 0003, lifetime
//! mechanism 2).

use std::sync::Arc;

use carrova_platform::Platform;
use tauri::utils::config::WindowConfig;
use tauri::wry::prelude::{JNIEnv, JObject};
use tauri::{AppHandle, Emitter, Manager, RunEvent, WebviewWindowBuilder};

use crate::opened::{OPENED_EVENT, Opened};
use crate::ops;

/// Open the window from the config (at start-up, and for the new activity
/// of an app reopened after its window was closed).
///
/// # Errors
///
/// No window in the config, or none can be built now (no activity yet).
pub fn open_main_window(app: &AppHandle) -> tauri::Result<()> {
    WebviewWindowBuilder::from_config(app, main_config(app)?)?.build()?;
    Ok(())
}

/// Closing the app ends its activity, not the process: the exit is refused
/// (the link service keeps the supervisor running) and the next activity
/// gets the window again. Links the app is opened with go to the UI.
pub fn on_run_event(app: &AppHandle, event: &RunEvent) {
    match event {
        RunEvent::ExitRequested {
            code: None, api, ..
        } => api.prevent_exit(),
        RunEvent::MainEventsCleared => reopen(app),
        RunEvent::Opened { urls } => {
            tracing::info!(links = urls.len(), "the app was opened with links");
            app.state::<Opened>()
                .push(urls.iter().map(ToString::to_string));
            if let Err(error) = app.emit(OPENED_EVENT, ()) {
                tracing::warn!(%error, "could not tell the UI of opened links");
            }
        }
        _ => {}
    }
}

/// Give a reopened app its window back. Tauri drops the window with the
/// activity and has no event for a new one, so every turn of the event
/// loop without a window tries: it fails at once until an activity exists.
fn reopen(app: &AppHandle) {
    let Ok(config) = main_config(app) else {
        return;
    };
    if app.get_webview_window(&config.label).is_some() {
        return;
    }
    match WebviewWindowBuilder::from_config(app, config).and_then(WebviewWindowBuilder::build) {
        Ok(_) => tracing::debug!("the reopened app has its window again"),
        Err(error) => tracing::trace!(%error, "no activity for the window yet"),
    }
}

fn main_config(app: &AppHandle) -> tauri::Result<&WindowConfig> {
    app.config()
        .app
        .windows
        .first()
        .ok_or(tauri::Error::WindowNotFound)
}

/// `LinkService.startSupervisor(context)` (Kotlin,
/// `org.byteveda.carrova.LinkService`): start this process's link
/// supervisor, before or without any activity.
// ADR 0003 (user sign-off 2026-11-15): src-tauri's one exported JNI
// function. `no_mangle` is what `unsafe_code` flags (the symbol name must
// be unique in the process); the JNI naming scheme needs the non-snake name.
#[allow(unsafe_code, non_snake_case)]
#[unsafe(no_mangle)]
pub extern "C" fn Java_org_byteveda_carrova_LinkService_startSupervisor(
    env: JNIEnv<'_>,
    _service: JObject<'_>,
    context: JObject<'_>,
) {
    start_link_service(&env, &context);
}

/// Hand the service's context to the platform FIRST: until then every
/// platform call (even [`carrova_platform::current`]) panics, which aborts
/// the process inside this JNI call. Then start the supervisor off the
/// service's main thread (it blocks; the same start as the app's `setup`,
/// so it runs once per process).
fn start_link_service(env: &JNIEnv<'_>, context: &JObject<'_>) {
    if let Err(error) = carrova_platform::attach_service_context(env, context) {
        tracing::error!(%error, "the link service cannot reach Android");
        return;
    }
    let started = std::thread::Builder::new()
        .name("carrova-link-start".to_owned())
        .spawn(|| {
            let platform: Arc<dyn Platform> = Arc::from(carrova_platform::current());
            if let Err(error) = ops::ecosystem::start_process_link(&platform) {
                tracing::warn!(%error, "linked devices are unavailable");
            }
        });
    if let Err(error) = started {
        tracing::error!(%error, "the link supervisor did not start");
    }
}
