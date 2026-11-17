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

use carrova_core::ecosystem::{CallView, ClipSettings, PhoneSettings, RingHere, SyncWindow};
use carrova_core::lan::{Inventory, PlanSummary};
use carrova_core::plan::SpaceNeed;
use carrova_core::{
    ArchiveSummary, Checklist, CodeOffer, Event, KdfStrength, Notice, PickFacts, PickKind,
    PickOutcome, PickRejection, PickResult, Plan, PlanEdit, PlanOptions, Preset, RollbackPreview,
    RollbackReport, ScanOptions, ScanReport, Selection, SelectionEdit, SelectionIssue, SendReport,
    SessionRecord,
};
use carrova_link::{Feature, FeatureState, Group};
use carrova_platform::FileManagerStatus;
use carrova_platform::phone::{
    AudioSide, CallAudioStatus, CallKind, CallRecord, CallState, Contact, Cursor, Hidden, Note,
    PermState, PhonePerm, SendOutcome, SmsMessage, SmsThread,
};
use carrova_report::{ErrorReport, IssueDraft, ReportConsent, ReportId, ReportSummary};
use carrova_view::devices::{
    ForgetDevices, KnownDeviceView, NearbyDeviceView, Ring, VisibilityChoice, VisibilityView,
};
use carrova_view::ecosystem::{
    AskTopic, AskView, ClipHistoryView, ClipItemView, ClipKind, FeatureRowView, LinkedDeviceView,
    PairInviteView, PairPeerView, TransferState, TransferView,
};
use carrova_view::ipc::{
    About, AlertChannel, AlertSettings, BackgroundSettings, BridgeError, BridgeEvent, CallAction,
    CallOutcome, CatalogEntry, EngineDefaults, NearbySettings, NoteAction, NoteOutcome, Picked,
    PlanReview, RecipesCheck, RecipesStatus, RecipesUpdate, ScanResult, SessionOpened, Settings,
    UpdateStatus,
};
use carrova_view::lan::{CodeView, InventoryView, NearbyPeer, PauseView, SummaryView};
use carrova_view::offer::OfferedItem;
use carrova_view::phone::{
    CallLogGroup, CallLogView, CallRow, CallSetupStep, CallWindowView, Caller, LocalClock,
    MessageRow, NoteView, NotesView, NotifyAppRow, NotifyAppsView, PermStep, SimChoice, ThreadRow,
    ThreadView, ThreadsView,
};
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
        NearbyDeviceView::export_all,
        Ring::export_all,
        KnownDeviceView::export_all,
        VisibilityView::export_all,
        VisibilityChoice::export_all,
        FeatureRowView::export_all,
        Feature::export_all,
        FeatureState::export_all,
        Group::export_all,
        PairInviteView::export_all,
        PairPeerView::export_all,
        ClipHistoryView::export_all,
        ClipItemView::export_all,
        ClipKind::export_all,
        LinkedDeviceView::export_all,
        AskView::export_all,
        AskTopic::export_all,
        TransferView::export_all,
        TransferState::export_all,
        ClipSettings::export_all,
        PhoneSettings::export_all,
        RingHere::export_all,
        SyncWindow::export_all,
        CallView::export_all,
        Note::export_all,
        Hidden::export_all,
        SmsThread::export_all,
        SmsMessage::export_all,
        CallState::export_all,
        CallRecord::export_all,
        Contact::export_all,
        CallKind::export_all,
        Cursor::export_all,
        SendOutcome::export_all,
        AudioSide::export_all,
        CallAudioStatus::export_all,
        PhonePerm::export_all,
        PermState::export_all,
        LocalClock::export_all,
        NotesView::export_all,
        NoteView::export_all,
        ThreadsView::export_all,
        ThreadRow::export_all,
        ThreadView::export_all,
        MessageRow::export_all,
        CallWindowView::export_all,
        Caller::export_all,
        CallLogView::export_all,
        CallLogGroup::export_all,
        CallRow::export_all,
        CallSetupStep::export_all,
        PermStep::export_all,
        NotifyAppsView::export_all,
        NotifyAppRow::export_all,
        SimChoice::export_all,
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
        ForgetDevices::export_all,
        NoteAction::export_all,
        NoteOutcome::export_all,
        CallAction::export_all,
        CallOutcome::export_all,
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
