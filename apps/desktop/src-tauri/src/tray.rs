//! The main window in background mode, and the tray icon that stands in
//! for it while it is closed. What to do comes from [`crate::ops::background`];
//! this only applies it to the window and the tray.
//!
//! Closing the window to the tray destroys it (its webview's memory goes
//! with it); opening recreates it under the same label from the app
//! config, and the UI loads its state afresh. While a move runs the window
//! is only hidden (its screen cannot be rebuilt), and closed for real once
//! the bridge goes idle. The tray icon exists only while the window is
//! closed or hidden, and keeps the app from exiting.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use carrova_core::time::now_unix;
use carrova_view::devices::VisibilityChoice;
use carrova_view::ipc::{BridgeEvent, EVENT_CHANNEL};
use carrova_view::tray::{
    CloseAction, TrayItem, TrayModel, tray_menu, tray_tooltip, tray_visibility,
};
use tauri::menu::{CheckMenuItem, IsMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::TrayIconBuilder;
use tauri::{
    AppHandle, Listener, Manager, RunEvent, WebviewWindow, WebviewWindowBuilder, Window,
    WindowEvent,
};
use tauri_plugin_dialog::DialogExt;

use crate::ops::{background, devices};
use crate::send_request;
use crate::state::AppState;

/// Launch flag of start at sign-in: start in the background, no window.
pub const BACKGROUND_FLAG: &str = "--background";

/// The label of the app's one window (also the tray icon's id).
const MAIN_WINDOW: &str = "main";

/// Whether the tray icon shows, whether its menu says a move runs, and how
/// many devices it last found nearby. It is built once, then only hidden:
/// Tauri keeps a handle to it, so dropping ours would not remove it.
#[derive(Default)]
struct TrayShown {
    shown: AtomicBool,
    moving: AtomicBool,
    nearby: Mutex<Option<u32>>,
    /// The "everyone" deadline a menu rebuild is waiting for (0: none).
    rebuild_at: AtomicU64,
}

/// Start-up: open the window, or with `--background` do what closing it
/// would (to the tray or minimised; with "Keep running" off, open it).
pub fn start(app: &AppHandle, state: &AppState, in_background: bool) {
    app.manage(TrayShown::default());
    let handle = app.clone();
    app.listen_any(EVENT_CHANNEL, move |_| on_bridge_event(&handle));
    let action = if in_background {
        background::close_action(state)
    } else {
        CloseAction::Exit
    };
    // Nothing runs yet, so there is no window to keep: straight to the tray.
    if matches!(action, CloseAction::CloseToTray | CloseAction::HideToTray)
        && show_tray(app, state).is_ok()
    {
        count_nearby(app);
        return;
    }
    let window = open_main(app);
    if action != CloseAction::Exit
        && let Some(window) = window
        && let Err(error) = window.minimize()
    {
        tracing::warn!(%error, "could not minimise the window");
    }
}

/// Unminimise, show and focus the main window, recreating it if it was
/// closed to the tray; the tray icon then hides. If the window cannot be
/// had, the tray icon stays (or comes back), so Carrova never vanishes.
pub fn open_main(app: &AppHandle) -> Option<WebviewWindow> {
    let window = match app.get_webview_window(MAIN_WINDOW) {
        Some(window) => window,
        None => match build_main(app) {
            Ok(window) => window,
            Err(error) => {
                tracing::warn!(%error, "could not open the window");
                if let Some(state) = app.try_state::<AppState>() {
                    // Logged inside; nothing more to fall back on.
                    let _ = show_tray(app, &state);
                }
                return None;
            }
        },
    };
    // Only now that the window exists, and before it shows (a shown window
    // with the tray up reads as hidden while busy).
    if let Some(tray) = app.tray_by_id(MAIN_WINDOW)
        && let Err(error) = tray.set_visible(false)
    {
        tracing::warn!(%error, "could not hide the tray icon");
    }
    set_tray_shown(app, false);
    if let Err(error) = window.unminimize().and_then(|()| window.show()) {
        tracing::warn!(%error, "could not show the window");
        // Still hidden: bring the tray back, or nothing would reach the app.
        if let Some(state) = app.try_state::<AppState>() {
            // Logged inside; nothing more to fall back on.
            let _ = show_tray(app, &state);
        }
    } else if let Err(error) = window.set_focus() {
        tracing::warn!(%error, "could not bring the window to the front");
    }
    Some(window)
}

/// Closing the main window: to the tray (closed, or hidden while a move
/// runs), minimised, or quit.
pub fn on_window_event(window: &Window, event: &WindowEvent) {
    let WindowEvent::CloseRequested { api, .. } = event else {
        return;
    };
    let app = window.app_handle();
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let action = background::close_action(&state);
    let tray = matches!(action, CloseAction::CloseToTray | CloseAction::HideToTray)
        && show_tray(app, &state).is_ok();
    if tray {
        count_nearby(app);
    }
    // Closed, the tray keeps the app running (`on_run_event`).
    if (tray && action == CloseAction::CloseToTray) || action == CloseAction::Exit {
        return;
    }
    api.prevent_close();
    let hidden = if tray {
        window.hide()
    } else {
        window.minimize()
    };
    if let Err(error) = hidden {
        tracing::warn!(%error, "could not hide the window");
    }
}

/// On each bridge event while the tray shows: follow a move starting or
/// ending, and close a window hidden while one ran (window and tray both
/// up) once nothing runs, which decides anew.
fn on_bridge_event(app: &AppHandle) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    if !tray_shown(app) {
        return;
    }
    let busy = state.sessions.busy();
    if busy != tray_flags(app).is_some_and(|t| t.moving.load(Ordering::SeqCst)) {
        // Logged inside; the icon stays as it was.
        let _ = show_tray(app, &state);
    }
    // Not while busy, nor when "lean" is off and the window is kept.
    if background::close_action(&state) != CloseAction::CloseToTray {
        return;
    }
    if let Some(window) = app.get_webview_window(MAIN_WINDOW)
        && let Err(error) = window.close()
    {
        tracing::warn!(%error, "could not close the hidden window");
    }
}

/// The last window closed: keep running while the tray icon is up. Quit
/// and restart pass an exit code and always go through.
pub fn on_run_event(app: &AppHandle, event: &RunEvent) {
    if let RunEvent::ExitRequested {
        code: None, api, ..
    } = event
        && tray_shown(app)
    {
        api.prevent_exit();
    }
}

fn build_main(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|w| w.label == MAIN_WINDOW)
        .ok_or_else(|| tauri::Error::WindowNotFound)?;
    WebviewWindowBuilder::from_config(app, config)?.build()
}

/// Show the tray icon with the menu of the moment (or refresh its menu).
fn show_tray(app: &AppHandle, state: &AppState) -> tauri::Result<()> {
    let shown = set_menu(app, &background::tray_model(state)).and_then(|()| {
        app.tray_by_id(MAIN_WINDOW)
            .map_or(Ok(()), |tray| tray.set_visible(true))
    });
    set_tray_shown(app, shown.is_ok());
    if let Err(error) = &shown {
        tracing::warn!(%error, "could not show the tray icon");
    }
    shown
}

/// Look for devices nearby off the main thread, then show the count in the
/// tray menu (if it still shows).
fn count_nearby(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let Some(state) = app.try_state::<AppState>() else {
            return;
        };
        let count = background::nearby_count(&state);
        if let Some(flags) = tray_flags(&app) {
            *flags.nearby.lock().unwrap_or_else(PoisonError::into_inner) = count;
        }
        if tray_shown(&app) {
            // Logged inside; the menu stays as it was.
            let _ = show_tray(&app, &state);
        }
    });
}

fn tray_flags(app: &AppHandle) -> Option<tauri::State<'_, TrayShown>> {
    app.try_state::<TrayShown>()
}

fn tray_shown(app: &AppHandle) -> bool {
    tray_flags(app).is_some_and(|t| t.shown.load(Ordering::SeqCst))
}

fn set_tray_shown(app: &AppHandle, shown: bool) {
    if let Some(flags) = tray_flags(app) {
        flags.shown.store(shown, Ordering::SeqCst);
    }
}

/// Rebuild the menu once "everyone" ends, so "Visible to" ticks the
/// choice then in force (once per deadline).
fn rebuild_when_everyone_ends(app: &AppHandle) {
    let (Some(state), Some(flags)) = (app.try_state::<AppState>(), tray_flags(app)) else {
        return;
    };
    let Some(until) = background::everyone_until(&state) else {
        return;
    };
    if flags.rebuild_at.swap(until, Ordering::SeqCst) == until {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        // A second past the deadline, so it reads as over.
        let wait = until.saturating_sub(now_unix()).saturating_add(1);
        std::thread::sleep(Duration::from_secs(wait));
        if let Some(state) = app.try_state::<AppState>()
            && tray_shown(&app)
        {
            // Logged inside; the menu stays as it was.
            let _ = show_tray(&app, &state);
        }
    });
}

/// Show `model` in the tray, with the tray's own nearby count.
fn set_menu(app: &AppHandle, model: &TrayModel) -> tauri::Result<()> {
    let flags = tray_flags(app);
    let model = &TrayModel {
        nearby_count: flags
            .as_ref()
            .and_then(|f| *f.nearby.lock().unwrap_or_else(PoisonError::into_inner)),
        ..model.clone()
    };
    let menu = menu(app, model)?;
    if let Some(flags) = flags {
        flags.moving.store(model.moving, Ordering::SeqCst);
    }
    if model.visible_to == VisibilityChoice::Everyone {
        rebuild_when_everyone_ends(app);
    }
    if let Some(tray) = app.tray_by_id(MAIN_WINDOW) {
        tray.set_tooltip(Some(tray_tooltip(model)))?;
        return tray.set_menu(Some(menu));
    }
    let mut tray = TrayIconBuilder::with_id(MAIN_WINDOW)
        .tooltip(tray_tooltip(model))
        .menu(&menu)
        .on_menu_event(|app, event| on_menu(app, event.id().as_ref()));
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app).map(drop)
}

fn menu(app: &AppHandle, model: &TrayModel) -> tauri::Result<Menu<tauri::Wry>> {
    let menu = Menu::new(app)?;
    for item in tray_menu(model) {
        if item.separated {
            menu.append(&PredefinedMenuItem::separator(app)?)?;
        }
        if item.children.is_empty() {
            menu.append(&*entry(app, &item)?)?;
            continue;
        }
        let submenu = Submenu::with_id(app, item.id, label(&item), item.enabled)?;
        for child in &item.children {
            submenu.append(&*entry(app, child)?)?;
        }
        menu.append(&submenu)?;
    }
    Ok(menu)
}

/// A plain or check menu item for `item`.
fn entry(app: &AppHandle, item: &TrayItem) -> tauri::Result<Box<dyn IsMenuItem<tauri::Wry>>> {
    Ok(match item.checked {
        Some(checked) => Box::new(CheckMenuItem::with_id(
            app,
            item.id,
            label(item),
            item.enabled,
            checked,
            None::<&str>,
        )?),
        None => Box::new(MenuItem::with_id(
            app,
            item.id,
            label(item),
            item.enabled,
            None::<&str>,
        )?),
    })
}

/// `item`'s label for a native menu: `&` marks a keyboard mnemonic there;
/// `&&` is a literal one.
fn label(item: &TrayItem) -> String {
    item.label.replace('&', "&&")
}

fn on_menu(app: &AppHandle, id: &str) {
    if let Some(choice) = tray_visibility(id) {
        set_visibility(app, choice);
        return;
    }
    match id {
        "open" => {
            open_main(app);
        }
        "nearby" => {
            // shortcut: a window built fresh here (lean mode) is not listening
            // yet and opens on Home; queue it like send requests if that matters.
            if open_main(app).is_some()
                && let Some(state) = app.try_state::<AppState>()
            {
                state.out.emit(&BridgeEvent::ShowNearby);
            }
        }
        "send" => pick_and_send(app),
        "pause" => {
            let Some(state) = app.try_state::<AppState>() else {
                return;
            };
            match background::toggle_pause(&state) {
                Ok(model) => {
                    if let Err(error) = set_menu(app, &model) {
                        tracing::warn!(%error, "could not refresh the tray menu");
                    }
                }
                Err(error) => tracing::warn!(%error, "could not pause alerts"),
            }
        }
        "quit" => app.exit(0),
        // `moving` is a status line; `review` and `decline` come with requests.
        _ => {}
    }
}

/// A "Visible to" choice: save it (the advert follows), then tick it.
fn set_visibility(app: &AppHandle, choice: VisibilityChoice) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    if let Err(error) = devices::apply_visibility(&state, choice) {
        tracing::warn!(%error, "could not change who can see this computer");
    }
    if let Err(error) = set_menu(app, &background::tray_model(&state)) {
        tracing::warn!(%error, "could not refresh the tray menu");
    }
}

/// "Send files…": pick files, then open the window on a send of them
/// (through the same queue as file-manager sends).
fn pick_and_send(app: &AppHandle) {
    let handle = app.clone();
    app.dialog().file().pick_files(move |picked| {
        let paths: Vec<PathBuf> = picked
            .unwrap_or_default()
            .into_iter()
            .filter_map(|path| path.into_path().ok())
            .collect();
        if paths.is_empty() {
            return;
        }
        if let Some(state) = handle.try_state::<AppState>() {
            send_request::forward(&state, &paths);
        }
        open_main(&handle);
    });
}
