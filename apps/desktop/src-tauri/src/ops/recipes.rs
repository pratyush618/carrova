//! Recipes: the catalogue, which recipes are in use, bundle updates and
//! reset.

use carrova_view::ipc::{CatalogEntry, RecipesCheck, RecipesStatus, RecipesUpdate};

use crate::command_error::CommandError;
use crate::state::AppState;
use crate::worker::blocking;

/// Apps Carrova knows, with detection on this machine.
///
/// # Errors
///
/// No recipes available.
pub async fn catalog(state: &AppState) -> Result<Vec<CatalogEntry>, CommandError> {
    let recipes = state.recipes.clone();
    blocking(move || recipes.catalog()).await
}

/// Source, version and counts of the recipes in use.
///
/// # Errors
///
/// No recipes available.
pub async fn status(state: &AppState) -> Result<RecipesStatus, CommandError> {
    let recipes = state.recipes.clone();
    blocking(move || recipes.status()).await
}

/// Check for, verify and install a newer recipe bundle. Only this
/// command (the user's click, or the UI's opt-in auto check) updates
/// recipes.
///
/// # Errors
///
/// Download or verification failure.
pub async fn update(state: &AppState) -> Result<RecipesUpdate, CommandError> {
    let recipes = state.recipes.clone();
    blocking(move || recipes.update()).await
}

/// Whether a newer recipe bundle is offered, without installing it.
///
/// # Errors
///
/// Download or verification failure.
pub async fn check(state: &AppState) -> Result<RecipesCheck, CommandError> {
    let recipes = state.recipes.clone();
    blocking(move || recipes.check()).await
}

/// Remove the installed recipe bundle and go back to the recipes shipped
/// with the app; returns the recipes now in use. Only this command (the
/// user's click in Settings) resets recipes.
///
/// # Errors
///
/// The store cannot be changed, or no recipes are available after it.
pub async fn reset(state: &AppState) -> Result<RecipesStatus, CommandError> {
    let recipes = state.recipes.clone();
    blocking(move || recipes.reset()).await
}
