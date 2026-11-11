//! The bridge's operations: one plain async function per `CarrovaPort`
//! method, over [`crate::state::AppState`]. Tauri commands
//! (`crate::commands`) only forward to them, so tests drive these
//! directly without a window.
//!
//! Each operation adapts IPC input to engine calls on the right thread
//! and engine results to IPC output; policy stays in the engine and the
//! shared view crate.

pub mod about;
pub mod inbox;
pub mod integration;
pub mod receive;
pub mod recipes;
pub mod send;
pub mod sessions;
pub mod settings;

use carrova_core::SessionId;
use carrova_view::ipc::BridgeEvent;

use crate::command_error::CommandError;
use crate::state::AppState;

/// Report the end of a long-running task of `session` to the UI
/// ([`BridgeEvent::TaskEnded`]) and pass its result on.
fn task_ended<T>(
    state: &AppState,
    session: &SessionId,
    result: Result<T, CommandError>,
) -> Result<T, CommandError> {
    state.out.emit(&BridgeEvent::TaskEnded {
        session: session.clone(),
        error: result.as_ref().err().map(|e| e.bridge().clone()),
    });
    result
}

#[cfg(test)]
#[path = "../../tests/ops/unit.rs"]
mod tests;
