//! The notification inbox: what is kept for the bell, and what is shown
//! beyond it (pop-up, screen wake) by the user's alert preferences.

use carrova_core::{Notice, NoticeId, NoticeKind, time};
use carrova_view::ipc::{AlertSettings, BridgeEvent, ErrorCode};
use carrova_view::notice::route;

use crate::command_error::CommandError;
use crate::state::AppState;

/// Title of every pop-up.
const POPUP_TITLE: &str = "Carrova";

/// The notices, newest first.
///
/// # Errors
///
/// Never in practice (the list is in memory); kept fallible like every op.
pub async fn list(state: &AppState) -> Result<Vec<Notice>, CommandError> {
    Ok(state.inbox.list())
}

/// Mark the notices `ids` as read (`None`: all of them).
///
/// # Errors
///
/// The inbox cannot be written.
pub async fn mark_read(state: &AppState, ids: Option<Vec<NoticeId>>) -> Result<(), CommandError> {
    match ids {
        Some(ids) => state.inbox.mark_read(&ids)?,
        None => state.inbox.mark_all_read()?,
    }
    Ok(())
}

/// Keep a notice for the bell, tell the UI, and pop up or wake the screen
/// as the preferences say. `summary` continues `peer`'s name when there
/// is one. A failure here is logged, never the caller's problem.
pub fn notify(state: &AppState, kind: NoticeKind, peer: Option<String>, summary: String) {
    let now = time::now_unix();
    let notice = match state.inbox.push(kind, now, peer, summary) {
        Ok(notice) => notice,
        Err(error) => {
            tracing::warn!(%error, "could not keep a notification");
            return;
        }
    };
    state.out.emit(&BridgeEvent::Notice {
        notice: notice.clone(),
    });
    let prefs = state.settings.load().map_or_else(
        |error| {
            tracing::warn!(error = %error.bridge().message, "alert settings unreadable: using the defaults");
            AlertSettings::default()
        },
        |settings| settings.alerts,
    );
    let delivery = route(kind, &prefs, now, state.platform.screen_locked());
    if delivery.popup {
        state
            .out
            .popup(POPUP_TITLE, &popup_body(&notice, delivery.show_names));
    }
    if delivery.wake
        && let Err(error) = state.platform.wake_display()
    {
        tracing::debug!(%error, "could not wake the display");
    }
}

/// Pass `result` of a finished move through, noting how it ended (a
/// cancelled move is the user's own doing: no news).
pub fn moved<T>(state: &AppState, result: Result<T, CommandError>) -> Result<T, CommandError> {
    match &result {
        Ok(_) => notify(
            state,
            NoticeKind::MoveFinished,
            None,
            "The move finished.".into(),
        ),
        Err(e) if e.bridge().code == ErrorCode::Cancelled => {}
        Err(_) => notify(
            state,
            NoticeKind::MoveFailed,
            None,
            "The move failed.".into(),
        ),
    }
    result
}

/// The pop-up text: the peer's name only where the preferences allow.
fn popup_body(notice: &Notice, show_names: bool) -> String {
    match (&notice.peer, show_names) {
        (Some(peer), true) => format!("{peer} {}", notice.summary),
        (Some(_), false) => format!("A device {}", notice.summary),
        (None, _) => notice.summary.clone(),
    }
}
