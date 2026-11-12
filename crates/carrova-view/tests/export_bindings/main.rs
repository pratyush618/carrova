//! Writes the TypeScript bindings (ts-rs) of every type the desktop
//! front-end receives or sends — engine results, events, requests and
//! the view-models — into `CARROVA_BINDINGS_DIR` (default
//! `apps/desktop/src/bindings`), one `<Type>.ts` per type, plus
//! `constants.ts` with the engine's values the front-end starts from
//! ([`EngineDefaults`]), after removing the `.ts` files there (so renamed
//! types leave nothing stale).
//!
//! Ignored by default (it writes files): `just bindings` runs it, and
//! `just bindings-check` diffs a fresh export against the committed one.
//!
//! Integers: `u64`/`i64` become `number`, which is what Tauri's JSON IPC
//! actually delivers (a JSON number parsed by `JSON.parse`). Sizes, counts
//! and Unix-second timestamps stay far below 2^53; the one exception,
//! `PlannedAction::mtime_ns`, is display-only and loses sub-microsecond
//! precision in JavaScript (front-ends never send plans back).
#![cfg(feature = "ts")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};

use carrova_core::lan::{Inventory, PlanSummary};
use carrova_core::plan::SpaceNeed;
use carrova_core::{
    ArchiveSummary, Checklist, CodeOffer, Event, KdfStrength, Notice, PickFacts, PickKind,
    PickOutcome, PickRejection, PickResult, Plan, PlanEdit, PlanOptions, Preset, RollbackPreview,
    RollbackReport, ScanOptions, ScanReport, Selection, SelectionEdit, SelectionIssue, SendReport,
    SessionRecord,
};
use carrova_platform::FileManagerStatus;
use carrova_report::{ErrorReport, IssueDraft, ReportConsent, ReportId, ReportSummary};
use carrova_view::ipc::{
    About, AlertChannel, AlertSettings, BackgroundSettings, BridgeError, BridgeEvent, CatalogEntry,
    EngineDefaults, NearbySettings, Picked, PlanReview, RecipesCheck, RecipesStatus, RecipesUpdate,
    ScanResult, SessionOpened, Settings, UpdateStatus,
};
use carrova_view::lan::{CodeView, InventoryView, NearbyPeer, PauseView, SummaryView};
use carrova_view::offer::OfferedItem;
use carrova_view::plan::{PlanView, SpaceView};
use carrova_view::selection::SelectionView;
use ts_rs::{Config, TS};

/// Where the bindings go.
fn out_dir() -> PathBuf {
    std::env::var_os("CARROVA_BINDINGS_DIR").map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join("../../apps/desktop/src/bindings"),
        PathBuf::from,
    )
}

/// Remove the generated `.ts` files of `dir` (other files stay).
fn clear(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "ts") {
            std::fs::remove_file(path).unwrap();
        }
    }
}

#[test]
#[ignore = "writes files: run through `just bindings`"]
fn export_typescript_bindings() {
    let dir = out_dir();
    clear(&dir);
    let cfg = Config::new().with_out_dir(&dir).with_large_int("number");
    let exports = [
        // Engine → front-end.
        Event::export_all,
        ScanReport::export_all,
        Plan::export_all,
        Checklist::export_all,
        SessionRecord::export_all,
        RollbackReport::export_all,
        RollbackPreview::export_all,
        SendReport::export_all,
        CodeOffer::export_all,
        ArchiveSummary::export_all,
        PlanSummary::export_all,
        Inventory::export_all,
        SpaceNeed::export_all,
        SelectionIssue::export_all,
        PickKind::export_all,
        PickFacts::export_all,
        PickRejection::export_all,
        PickResult::export_all,
        PickOutcome::export_all,
        // Front-end → engine.
        ScanOptions::export_all,
        Selection::export_all,
        SelectionEdit::export_all,
        Preset::export_all,
        PlanOptions::export_all,
        PlanEdit::export_all,
        KdfStrength::export_all,
        // View-models.
        OfferedItem::<'static>::export_all,
        SelectionView::<'static>::export_all,
        CodeView::export_all,
        NearbyPeer::export_all,
        InventoryView::<'static>::export_all,
        SummaryView::<'static>::export_all,
        PauseView::<'static>::export_all,
        PlanView::<'static>::export_all,
        SpaceView::export_all,
        // Desktop IPC contract.
        BridgeEvent::export_all,
        BridgeError::export_all,
        SessionOpened::export_all,
        RecipesStatus::export_all,
        RecipesUpdate::export_all,
        RecipesCheck::export_all,
        CatalogEntry::export_all,
        Settings::export_all,
        BackgroundSettings::export_all,
        AlertSettings::export_all,
        NearbySettings::export_all,
        AlertChannel::export_all,
        ScanResult::<'static>::export_all,
        PlanReview::<'static>::export_all,
        UpdateStatus::export_all,
        About::export_all,
        EngineDefaults::export_all,
        Picked::<'static>::export_all,
        FileManagerStatus::export_all,
        Notice::export_all,
        // Error reports (carrova-report).
        ErrorReport::export_all,
        ReportSummary::export_all,
        ReportId::export_all,
        ReportConsent::export_all,
        IssueDraft::export_all,
    ];
    for export in exports {
        export(&cfg).unwrap();
    }
    write_constants(&dir);
    assert!(dir.join("Event.ts").is_file());
}

/// `constants.ts`: [`EngineDefaults::current`] as a typed constant.
fn write_constants(dir: &Path) {
    let value = serde_json::to_string_pretty(&EngineDefaults::current()).unwrap();
    let text = format!(
        "// Generated by carrova-view's export_bindings test (`just bindings`). Do not edit.\n\
         import type {{ EngineDefaults }} from './EngineDefaults';\n\n\
         /** The engine's suggestions and policies (`EngineDefaults::current`). */\n\
         export const ENGINE_DEFAULTS: EngineDefaults = {value};\n"
    );
    std::fs::write(dir.join("constants.ts"), text).unwrap();
}
