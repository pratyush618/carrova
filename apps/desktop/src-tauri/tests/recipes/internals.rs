//! Desktop recipe internals: reset reports the recipes in use afterwards.
//! (The `CARROVA_RECIPES` policy is `carrova-bundle`'s
//! `DevOverride::DebugBuildsOnly`, tested there.)

use carrova_view::ipc::RecipeOrigin;
use tauri::async_runtime::block_on;

use crate::ops::recipes;
use crate::testing::Machine;

#[test]
fn reset_with_nothing_installed_reports_the_recipes_in_use() {
    let machine = Machine::new();
    let status = block_on(recipes::reset(&machine.state)).unwrap();
    assert!(matches!(status.origin, RecipeOrigin::DevDir));
}
