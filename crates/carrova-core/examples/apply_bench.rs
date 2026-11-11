//! Apply wall-time benchmark: 1000 × 2 KiB files, create then replace, on
//! the filesystem holding the system temp dir (use a real disk, not tmpfs).
//!
//! ```text
//! cargo run -p carrova-core --example apply_bench [-- FILES]
//! ```
//!
//! Prints timings only; nothing is asserted.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

use std::path::Path;
use std::time::Instant;

use carrova_core::plan::{Action, ObjectKind, Plan, PlanOptions, PlannedAction};
use carrova_core::session::{NullSink, SessionId};
use carrova_core::test_support::apply::{Failpoint, apply};
use carrova_core::test_support::stage::Staged;
use carrova_manifest::{
    Blake3Hash, Entry, EntryKind, ItemFlags, Manifest, ManifestItem, RelPath, SourceInfo,
};
use carrova_platform::fake::FakePlatform;
use carrova_platform::{OsKind, SemanticRoot};

const FILE_SIZE: usize = 2048;
const SID: &str = "bench";

fn main() {
    let files: usize = std::env::args()
        .nth(1)
        .map_or(1000, |n| n.parse().expect("FILES must be a number"));
    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path().join("home");
    std::fs::create_dir(&home).unwrap();
    let state = tmp.path().join("state");
    let platform = FakePlatform::with_home(&home).with_state_dir(&state);

    let create = run(&platform, &state, &home, "create", files, &Action::Create);
    let replace = run(&platform, &state, &home, "replace", files, &Action::Replace);
    println!(
        "apply {files} x {FILE_SIZE} B on {}: create {:.3} s, replace {:.3} s",
        tmp.path().display(),
        create,
        replace
    );
}

/// Stage `files` files in a fresh session, apply them, return seconds.
fn run(
    platform: &FakePlatform,
    state: &Path,
    home: &Path,
    name: &str,
    files: usize,
    act: &Action,
) -> f64 {
    let session_dir = state.join("sessions").join(name);
    let staging = session_dir.join("staging/files");
    std::fs::create_dir_all(&staging).unwrap();
    let item = ManifestItem {
        recipe_id: None,
        item_id: "bench".into(),
        label: "bench".into(),
        flags: ItemFlags::default(),
        entries: Vec::new(),
        handler: None,
        artifacts: Vec::new(),
        source_paths: Vec::new(),
    };
    let key = carrova_core::ItemKey::of(&item);
    let mut manifest = Manifest {
        format_version: 2,
        created_at: 0,
        source: SourceInfo {
            os: OsKind::Linux,
            carrova_version: "bench".into(),
            device_name: None,
        },
        items: vec![item],
        source_home: None,
    };
    let mut staged = Staged::default();
    let mut actions = Vec::with_capacity(files);
    for i in 0..files {
        let bytes = format!("{name}-{i}-").into_bytes().repeat(FILE_SIZE / 8);
        let bytes = &bytes[..FILE_SIZE];
        let src = staging.join(i.to_string());
        std::fs::write(&src, bytes).unwrap();
        staged.files.insert((key.clone(), i), src);
        let rel = format!("bench/d{}/f{i}", i % 10);
        manifest.items[0].entries.push(Entry {
            root: SemanticRoot::Home,
            rel: RelPath::new(rel.clone()).unwrap(),
            kind: EntryKind::File {
                size: FILE_SIZE as u64,
                hash: Blake3Hash::of(bytes),
                chunks: Vec::new(),
            },
            mode: Some(0o644),
            mtime_ns: Some(1_700_000_000_000_000_000),
        });
        actions.push(PlannedAction {
            item: key.clone(),
            entry_index: i,
            kind: ObjectKind::File,
            root: SemanticRoot::Home,
            abs: home.join(&rel),
            rel,
            size: FILE_SIZE as u64,
            mode: Some(0o644),
            link_target: None,
            mtime_ns: Some(1_700_000_000_000_000_000),
            action: act.clone(),
        });
    }
    let plan = Plan {
        session: SessionId(SID.into()),
        options: PlanOptions::default(),
        bytes_to_write: (files * FILE_SIZE) as u64,
        actions,
        handlers: Vec::new(),
        free_space: None,
        missing_apps: Vec::new(),
        running_apps: Vec::new(),
        app_names: Default::default(),
        mappings: Vec::new(),
        plugins: Vec::new(),
        blockers: Vec::new(),
        warnings: Vec::new(),
        sensitive: Default::default(),
        safe_picks: Vec::new(),
    };
    let start = Instant::now();
    apply(
        platform,
        &plan,
        &manifest,
        &staged,
        &session_dir,
        &NullSink,
        Failpoint::OFF,
    )
    .unwrap();
    start.elapsed().as_secs_f64()
}
