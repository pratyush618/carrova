//! The call window: a small always-on-top window for a linked phone's
//! call, opened when one rings here (the engine's `ring_here`) or runs,
//! and closed by its own UI once the call is over. The last `call_view`
//! is kept so a window opened mid-call, or the main window reopened, starts
//! from it ([`call_now`]), and so the tray can show the call. Plumbing
//! only: what a call shows comes from the engine's events.

use std::fmt;
use std::sync::{Mutex, PoisonError};

use carrova_core::ecosystem::CallView;
use carrova_crypto::identity::DeviceId;
use carrova_view::ipc::EVENT_CHANNEL;
use tauri::{AppHandle, Listener, Manager, State, WebviewUrl, WebviewWindowBuilder};

use crate::command_error::CommandError;
use crate::tray;

#[cfg(test)]
#[path = "../tests/call_window/unit.rs"]
mod tests;

/// The call window's label (the UI renders the call alone in it).
pub const CALL_WINDOW: &str = "call";

/// The last call the engine showed, and who rang (kept for the tray).
#[derive(Default)]
pub struct LastCall(Mutex<Shown>);

#[derive(Default)]
struct Shown {
    view: Option<CallView>,
    /// The caller's name, else number, from the ringing (memory only).
    who: Option<String>,
}

/// The running call as the tray shows it.
pub struct TrayCall {
    /// The phone on the call.
    pub device: DeviceId,
    /// Its microphone is muted.
    pub muted: bool,
    /// Who rang, if it rang here.
    pub who: Option<String>,
}

// The caller stays out of `Debug` (never logged).
impl fmt::Debug for LastCall {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("LastCall")
    }
}

impl fmt::Debug for TrayCall {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TrayCall")
            .field("device", &self.device)
            .field("muted", &self.muted)
            .finish_non_exhaustive()
    }
}

impl LastCall {
    fn take(&self, view: CallView) {
        let mut shown = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        match &view {
            CallView::Ringing { number, name, .. } | CallView::Missed { number, name, .. } => {
                shown.who = Some(name.clone().unwrap_or_else(|| number.clone()));
            }
            CallView::None | CallView::Left { .. } => shown.who = None,
            CallView::Active { .. } => {}
        }
        shown.view = Some(view);
    }

    fn view(&self) -> CallView {
        let shown = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        shown.view.clone().unwrap_or(CallView::None)
    }

    /// The running call, if one runs.
    pub fn running(&self) -> Option<TrayCall> {
        let shown = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        match &shown.view {
            Some(CallView::Active { device, muted, .. }) => Some(TrayCall {
                device: *device,
                muted: *muted,
                who: shown.who.clone(),
            }),
            _ => None,
        }
    }
}

/// Follow the engine's `call_view` events: keep the last, open the window
/// when a call rings here or runs, and refresh the tray.
pub fn start(app: &AppHandle) {
    app.manage(LastCall::default());
    let handle = app.clone();
    app.listen_any(EVENT_CHANNEL, move |event| {
        let Some(view) = call_view(event.payload()) else {
            return;
        };
        let open = opens_window(&view);
        if let Some(last) = handle.try_state::<LastCall>() {
            last.take(view);
        }
        tray::on_main(&handle, move |app| {
            if open {
                open_window(app);
            }
            tray::refresh_shown_tray(app);
        });
    });
}

/// `CarrovaPort.callNow`: the last call shown (`none` before any).
///
/// # Errors
///
/// None: async (Tauri then needs a `Result`) so the state isn't held by value.
#[tauri::command]
pub async fn call_now(last: State<'_, LastCall>) -> Result<CallView, CommandError> {
    Ok(last.view())
}

/// The call a bridge event payload shows, if it is a `call_view`.
fn call_view(payload: &str) -> Option<CallView> {
    let mut event: serde_json::Value = serde_json::from_str(payload).ok()?;
    if event["kind"] != "engine" || event["event"]["event"] != "call_view" {
        return None;
    }
    serde_json::from_value(event["event"].take()).ok()
}

/// Whether `view` opens the call window: a call ringing here, or running.
fn opens_window(view: &CallView) -> bool {
    matches!(
        view,
        CallView::Ringing {
            ring_here: true,
            ..
        } | CallView::Active { .. }
    )
}

/// Show the call window, building it if it is not open. It never takes
/// the keyboard: a key pressed mid-typing must not answer or decline.
fn open_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(CALL_WINDOW) {
        if let Err(error) = window.show() {
            tracing::warn!(%error, "could not show the call window");
        }
        return;
    }
    let built = WebviewWindowBuilder::new(app, CALL_WINDOW, WebviewUrl::App("index.html".into()))
        .title("Carrova")
        .inner_size(340.0, 400.0)
        .resizable(false)
        .always_on_top(true)
        .focused(false)
        .build();
    if let Err(error) = built {
        tracing::warn!(%error, "could not open the call window");
    }
}
