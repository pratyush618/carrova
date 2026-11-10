//! Recipe commands.

use carrova_view::ipc::{CatalogEntry, RecipesCheck, RecipesStatus, RecipesUpdate};
use tauri::State;

use crate::command_error::CommandError;
use crate::ops;
use crate::state::AppState;

/// `CarrovaPort.catalog`.
#[tauri::command]
pub async fn catalog(state: State<'_, AppState>) -> Result<Vec<CatalogEntry>, CommandError> {
    ops::recipes::catalog(&state).await
}

/// `CarrovaPort.recipesStatus`.
#[tauri::command]
pub async fn recipes_status(state: State<'_, AppState>) -> Result<RecipesStatus, CommandError> {
    ops::recipes::status(&state).await
}

/// `CarrovaPort.recipesUpdate`.
#[tauri::command]
pub async fn recipes_update(state: State<'_, AppState>) -> Result<RecipesUpdate, CommandError> {
    ops::recipes::update(&state).await
}

/// `CarrovaPort.recipesCheck`.
#[tauri::command]
pub async fn recipes_check(state: State<'_, AppState>) -> Result<RecipesCheck, CommandError> {
    ops::recipes::check(&state).await
}

/// `CarrovaPort.recipesReset`.
#[tauri::command]
pub async fn recipes_reset(state: State<'_, AppState>) -> Result<RecipesStatus, CommandError> {
    ops::recipes::reset(&state).await
}
