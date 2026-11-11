//! Receiver staging fast path: `StageLayout`, `StagingStore` (resume,
//! torn log, disk full, verification), `transfer_order` and the
//! per-filesystem space policy.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::{BTreeMap, HashMap};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

use carrova_core::lan::transfer_order;
use carrova_core::lan::{MIN_FREE_DURING_TRANSFER, check_space};
use carrova_core::plan::{
    Action, ObjectKind, Plan, PlanOptions, PlannedAction, required_space, space_needs,
};
use carrova_core::session::SessionId;
use carrova_core::test_support::stage::staging::{FILES_DIR, PROGRESS_LOG};
use carrova_core::test_support::stage::{CheckpointPolicy, StageLayout, StagingStore};
use carrova_core::{CoreError, ItemKey, ItemTarget};
use carrova_manifest::{
    Blake3Hash, ChunkHash, ChunkRef, Entry, EntryKind, ItemFlags, Manifest, ManifestItem, RelPath,
    SourceInfo,
};
use carrova_platform::fake::FakePlatform;
use carrova_platform::{OsKind, Platform, SemanticRoot};
use tempfile::TempDir;

// ---------------------------------------------------------------- fixtures

/// One file to stage: item key, target root, data, chunk size.
struct Spec<'a> {
    item: &'a str,
    root: SemanticRoot,
    data: Vec<u8>,
    piece: usize,
}

fn spec<'a>(item: &'a str, root: SemanticRoot, data: &[u8], piece: usize) -> Spec<'a> {
    Spec {
        item,
        root,
        data: data.to_vec(),
        piece,
    }
}

struct Fixture {
    plan: Plan,
    manifest: Manifest,
    /// Chunk hash → bytes.
    chunks: HashMap<ChunkHash, Vec<u8>>,
    /// Item key → entry index → data.
    data: BTreeMap<(ItemKey, usize), Vec<u8>>,
}

impl Fixture {
    fn layout(&self) -> StageLayout {
        StageLayout::new(&self.plan, &self.manifest).unwrap()
    }

    fn bytes(&self, c: &ChunkRef) -> &[u8] {
        &self.chunks[&c.hash]
    }
}

/// Fixed-size pieces: contiguous, so a valid tiling.
fn chunked(data: &[u8], piece: usize) -> (Vec<ChunkRef>, Vec<(ChunkHash, Vec<u8>)>) {
    let mut refs = Vec::new();
    let mut bytes = Vec::new();
    let mut offset = 0u64;
    for p in data.chunks(piece) {
        let hash = Blake3Hash::of(p);
        refs.push(ChunkRef {
            hash,
            offset,
            len: u32::try_from(p.len()).unwrap(),
        });
        bytes.push((hash, p.to_vec()));
        offset += p.len() as u64;
    }
    (refs, bytes)
}

/// Every spec becomes a `Create` of its own entry. Items keep first-seen
/// order; `raw:<root>/<rel>` selection keys become recipe-less items with
/// id `<root>/<rel>`, as capture makes them.
fn fixture(home: &Path, specs: &[Spec<'_>]) -> Fixture {
    let mut items: Vec<ManifestItem> = Vec::new();
    let mut actions = Vec::new();
    let mut chunks = HashMap::new();
    let mut data = BTreeMap::new();
    for s in specs {
        let (recipe_id, item_id) = match ItemKey::from(s.item).target().unwrap() {
            ItemTarget::Raw(path) => (None, path.raw_item_id()),
            ItemTarget::Recipe { recipe, item } => (Some(recipe.to_owned()), item.to_owned()),
        };
        let pos = items
            .iter()
            .position(|i| i.item_id == item_id && i.recipe_id == recipe_id)
            .unwrap_or_else(|| {
                items.push(ManifestItem {
                    recipe_id: recipe_id.clone(),
                    item_id: item_id.clone(),
                    label: item_id.clone(),
                    flags: ItemFlags::default(),
                    entries: Vec::new(),
                    handler: None,
                    artifacts: Vec::new(),
                    source_paths: Vec::new(),
                });
                items.len() - 1
            });
        let item = &mut items[pos];
        let idx = item.entries.len();
        let rel = format!("{}-{idx}", item_id.replace([':', '/'], "_"));
        let (refs, bytes) = chunked(&s.data, s.piece);
        chunks.extend(bytes);
        item.entries.push(Entry {
            root: s.root,
            rel: RelPath::new(rel.clone()).unwrap(),
            kind: EntryKind::File {
                size: s.data.len() as u64,
                hash: Blake3Hash::of(&s.data),
                chunks: refs,
            },
            mode: Some(0o644),
            mtime_ns: None,
        });
        actions.push(PlannedAction {
            item: ItemKey::of(item),
            entry_index: idx,
            kind: ObjectKind::File,
            root: s.root,
            rel: rel.clone(),
            abs: home.join(&rel),
            size: s.data.len() as u64,
            mode: Some(0o644),
            link_target: None,
            mtime_ns: None,
            action: Action::Create,
        });
        data.insert((s.item.into(), idx), s.data.clone());
    }
    let manifest = Manifest::new(
        0,
        SourceInfo {
            os: OsKind::Linux,
            carrova_version: "test".into(),
            device_name: None,
        },
        items,
    );
    manifest.validate().unwrap();
    let plan = Plan {
        session: SessionId("sess-1".into()),
        options: PlanOptions::default(),
        actions,
        handlers: Vec::new(),
        bytes_to_write: 0,
        free_space: None,
        missing_apps: Vec::new(),
        running_apps: Vec::new(),
        app_names: Default::default(),
        mappings: Vec::new(),
        plugins: Vec::new(),
        blockers: Vec::new(),
        warnings: Vec::new(),
        sensitive: BTreeMap::new(),
        safe_picks: Vec::new(),
    };
    Fixture {
        plan,
        manifest,
        chunks,
        data,
    }
}

struct Env {
    _tmp: TempDir,
    home: PathBuf,
    staging: PathBuf,
    platform: FakePlatform,
}

fn env_with(f: impl FnOnce(FakePlatform, &Path) -> FakePlatform) -> Env {
    let tmp = TempDir::new().unwrap();
    let home = tmp.path().join("home");
    let state = tmp.path().join("state");
    let session = state.join("sessions/sess-1");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(&session).unwrap();
    let platform = f(FakePlatform::with_home(&home).with_state_dir(&state), &home);
    Env {
        _tmp: tmp,
        home,
        staging: session.join("staging"),
        platform,
    }
}

fn env() -> Env {
    env_with(|p, _| p)
}

impl Env {
    fn open(&self, layout: StageLayout, policy: CheckpointPolicy) -> StagingStore {
        StagingStore::open(&self.platform, &self.staging, layout, policy).unwrap()
    }

    fn put_all(&self, store: &mut StagingStore, fx: &Fixture) {
        let all: Vec<ChunkRef> = store.layout().unique_chunks().collect();
        for c in all {
            store.put(&self.platform, &c, fx.bytes(&c)).unwrap();
        }
    }
}

fn read_staged(fx: &Fixture, staged: &carrova_core::test_support::stage::Staged) {
    for (key, want) in &fx.data {
        let path = &staged.files[key];
        assert_eq!(&std::fs::read(path).unwrap(), want, "{key:?}");
    }
}

fn two_files_sharing_a_chunk(home: &Path) -> Fixture {
    let shared = vec![b'S'; 1000];
    let a = [vec![b'a'; 1000], shared.clone(), vec![b'x'; 10]].concat();
    let b = [shared.clone(), vec![b'b'; 1000], shared].concat();
    fixture(
        home,
        &[
            spec("app/one", SemanticRoot::UserConfig, &a, 1000),
            spec("app/two", SemanticRoot::UserConfig, &b, 1000),
        ],
    )
}

// ------------------------------------------------------------------ layout

#[test]
fn shared_chunk_is_fetched_once_and_written_to_every_file() {
    let e = env();
    let fx = two_files_sharing_a_chunk(&e.home);
    let layout = fx.layout();
    let shared = Blake3Hash::of(&[b'S'; 1000]);
    // In file 0 once, in file 1 twice.
    assert_eq!(layout.places(&shared).len(), 3);
    // a, S, x, b — S counted once.
    assert_eq!(layout.unique_chunk_count(), 4);
    assert_eq!(layout.total_bytes(), 2010 + 3000);

    let mut store = e.open(layout, CheckpointPolicy::Periodic);
    e.put_all(&mut store, &fx);
    let staged = store.finish(&e.platform).unwrap();
    assert_eq!(staged.files.len(), 2);
    read_staged(&fx, &staged);
    for path in staged.files.values() {
        assert!(path.starts_with(e.staging.join(FILES_DIR)));
        if cfg!(unix) {
            assert_eq!(e.platform.metadata(path).unwrap().mode, Some(0o600));
        }
    }
}

#[test]
fn layout_skips_actions_that_write_no_content() {
    let e = env();
    let mut fx = fixture(
        &e.home,
        &[
            spec("app/one", SemanticRoot::UserConfig, b"one", 64),
            spec("app/one", SemanticRoot::UserConfig, b"two", 64),
            spec("app/one", SemanticRoot::UserConfig, b"three", 64),
        ],
    );
    fx.plan.actions[0].action = Action::Identical;
    fx.plan.actions[1].action = Action::Skip {
        reason: "user".into(),
    };
    let layout = fx.layout();
    assert_eq!(layout.files().len(), 1);
    assert_eq!(layout.total_bytes(), 5);

    // A plan naming an entry the manifest lacks is refused.
    fx.plan.actions[2].entry_index = 9;
    let err = StageLayout::new(&fx.plan, &fx.manifest).unwrap_err();
    assert!(matches!(err, CoreError::Verification(_)), "{err}");
}

#[test]
fn put_rejects_wrong_bytes_and_chunks_the_plan_does_not_need() {
    let e = env();
    let fx = two_files_sharing_a_chunk(&e.home);
    let mut store = e.open(fx.layout(), CheckpointPolicy::Periodic);
    let c = store.layout().unique_chunks().next().unwrap();
    let mut bad = fx.bytes(&c).to_vec();
    bad[0] ^= 1;
    assert!(matches!(
        store.put(&e.platform, &c, &bad),
        Err(CoreError::Verification(_))
    ));
    let stray = ChunkRef {
        hash: Blake3Hash::of(b"stray"),
        offset: 0,
        len: 5,
    };
    assert!(matches!(
        store.put(&e.platform, &stray, b"stray"),
        Err(CoreError::Verification(_))
    ));
    store.checkpoint(&e.platform).unwrap();
    assert!(!store.contains(&c.hash));
}

// ------------------------------------------------------------------ resume

#[test]
fn only_checkpointed_chunks_survive_a_crash() {
    let e = env();
    let fx = two_files_sharing_a_chunk(&e.home);
    let all: Vec<ChunkRef> = fx.layout().unique_chunks().collect();

    let mut store = e.open(fx.layout(), CheckpointPolicy::Periodic);
    store.put(&e.platform, &all[0], fx.bytes(&all[0])).unwrap();
    store.checkpoint(&e.platform).unwrap();
    store.put(&e.platform, &all[1], fx.bytes(&all[1])).unwrap();
    // Not checkpointed: not reported as staged yet.
    assert!(store.contains(&all[0].hash));
    assert!(!store.contains(&all[1].hash));
    drop(store); // crash

    let mut store = e.open(fx.layout(), CheckpointPolicy::Periodic);
    assert!(store.contains(&all[0].hash));
    assert_eq!(store.missing(&all), all[1..].to_vec());
    for c in &all[1..] {
        store.put(&e.platform, c, fx.bytes(c)).unwrap();
    }
    let staged = store.finish(&e.platform).unwrap();
    read_staged(&fx, &staged);
    assert!(e.staging.join(PROGRESS_LOG).is_file());

    // Finished staging resumes as complete (nothing left to fetch).
    let store = e.open(fx.layout(), CheckpointPolicy::Periodic);
    assert!(store.missing(&all).is_empty());
}

#[test]
fn missing_keeps_order_and_drops_duplicates() {
    let e = env();
    let fx = two_files_sharing_a_chunk(&e.home);
    let all: Vec<ChunkRef> = fx.layout().unique_chunks().collect();
    let mut store = e.open(fx.layout(), CheckpointPolicy::Periodic);
    store.put(&e.platform, &all[1], fx.bytes(&all[1])).unwrap();
    store.checkpoint(&e.platform).unwrap();
    let asked = vec![all[3], all[1], all[0], all[3], all[2]];
    assert_eq!(store.missing(&asked), vec![all[3], all[0], all[2]]);
}

#[test]
fn torn_progress_log_tail_is_dropped() {
    let e = env();
    let fx = two_files_sharing_a_chunk(&e.home);
    let all: Vec<ChunkRef> = fx.layout().unique_chunks().collect();
    let log = e.staging.join(PROGRESS_LOG);

    let mut store = e.open(fx.layout(), CheckpointPolicy::Periodic);
    store.put(&e.platform, &all[0], fx.bytes(&all[0])).unwrap();
    store.checkpoint(&e.platform).unwrap();
    drop(store);
    let intact = std::fs::metadata(&log).unwrap().len();

    // A record cut off mid-write: count says 2 hashes, only 1 + part made it.
    let mut torn = 2u32.to_le_bytes().to_vec();
    torn.extend_from_slice(all[1].hash.as_bytes());
    torn.extend_from_slice(&[0xAB; 7]);
    let mut f = std::fs::OpenOptions::new().append(true).open(&log).unwrap();
    f.write_all(&torn).unwrap();
    drop(f);

    let mut store = e.open(fx.layout(), CheckpointPolicy::Periodic);
    assert!(store.contains(&all[0].hash));
    assert!(!store.contains(&all[1].hash));
    assert_eq!(std::fs::metadata(&log).unwrap().len(), intact);

    // A complete record with a bad checksum is dropped too.
    store.put(&e.platform, &all[1], fx.bytes(&all[1])).unwrap();
    store.checkpoint(&e.platform).unwrap();
    drop(store);
    let mut bytes = std::fs::read(&log).unwrap();
    let last = bytes.len() - 1;
    bytes[last] ^= 0xFF;
    std::fs::write(&log, &bytes).unwrap();
    let store = e.open(fx.layout(), CheckpointPolicy::Periodic);
    assert!(store.contains(&all[0].hash));
    assert!(!store.contains(&all[1].hash));
}

#[test]
fn changed_layout_wipes_stale_staging() {
    let e = env();
    let fx = two_files_sharing_a_chunk(&e.home);
    let mut store = e.open(fx.layout(), CheckpointPolicy::Periodic);
    e.put_all(&mut store, &fx);
    store.checkpoint(&e.platform).unwrap();
    drop(store);

    // The user re-planned: only the first file is wanted now.
    let mut replanned = two_files_sharing_a_chunk(&e.home);
    replanned.plan.actions[1].action = Action::Skip {
        reason: "user".into(),
    };
    let store = e.open(replanned.layout(), CheckpointPolicy::Periodic);
    let all: Vec<ChunkRef> = replanned.layout().unique_chunks().collect();
    assert_eq!(store.missing(&all), all);
    let files: Vec<_> = std::fs::read_dir(e.staging.join(FILES_DIR))
        .unwrap()
        .map(|d| d.unwrap().file_name())
        .collect();
    assert_eq!(files, ["0"]);
}

// ------------------------------------------------------------ verification

#[test]
fn finish_detects_a_corrupted_staged_file() {
    let e = env();
    let fx = two_files_sharing_a_chunk(&e.home);
    let mut store = e.open(fx.layout(), CheckpointPolicy::Periodic);
    e.put_all(&mut store, &fx);
    store.checkpoint(&e.platform).unwrap();
    let f = e.staging.join(FILES_DIR).join("1");
    let mut bytes = std::fs::read(&f).unwrap();
    bytes[1500] ^= 1;
    std::fs::write(&f, &bytes).unwrap();
    let err = store.finish(&e.platform).unwrap_err();
    assert!(
        matches!(&err, CoreError::Verification(what) if what == "app/two#0"),
        "{err}"
    );
}

#[test]
fn finish_refuses_while_chunks_are_missing() {
    let e = env();
    let fx = two_files_sharing_a_chunk(&e.home);
    let mut store = e.open(fx.layout(), CheckpointPolicy::OnFinish);
    let first = store.layout().unique_chunks().next().unwrap();
    store.put(&e.platform, &first, fx.bytes(&first)).unwrap();
    let err = store.finish(&e.platform).unwrap_err();
    assert!(err.to_string().contains("never arrived"), "{err}");
}

// --------------------------------------------------------------- disk full

#[test]
fn preallocation_beyond_free_space_is_disk_full_and_frees_staging() {
    let e = env_with(|p, _| p.free_space_bytes(3000));
    let fx = two_files_sharing_a_chunk(&e.home);
    // The first file fits, the second does not.
    let big = fixture(
        &e.home,
        &[
            spec("app/one", SemanticRoot::UserConfig, &[1u8; 2000], 1000),
            spec("app/two", SemanticRoot::UserConfig, &[2u8; 4000], 1000),
        ],
    );
    let err = StagingStore::open(
        &e.platform,
        &e.staging,
        big.layout(),
        CheckpointPolicy::Periodic,
    )
    .unwrap_err();
    match err {
        CoreError::DiskFull { needed, available } => {
            assert_eq!(needed, 4000, "bytes still to allocate");
            assert_eq!(available, 3000);
        }
        other => panic!("{other:?}"),
    }
    assert!(!e.staging.exists(), "partial staging wiped");
    // Smaller plan fits.
    e.open(fx.layout(), CheckpointPolicy::Periodic);
}

#[test]
fn check_space_keeps_headroom_for_log_and_journal() {
    let low = MIN_FREE_DURING_TRANSFER - 1;
    let e = env_with(|p, _| p);
    let staging_fs = e.staging.parent().unwrap().to_path_buf();
    let fx = two_files_sharing_a_chunk(&e.home);
    let all: Vec<ChunkRef> = fx.layout().unique_chunks().collect();

    let tight = e.platform.clone().filesystem(&staging_fs, 1, low);
    match check_space(&tight, &e.staging, &all).unwrap_err() {
        CoreError::DiskFull { needed, available } => {
            assert_eq!(available, low);
            assert_eq!(needed, MIN_FREE_DURING_TRANSFER + 32 * all.len() as u64);
        }
        other => panic!("{other:?}"),
    }
    let roomy = e.platform.clone().filesystem(&staging_fs, 1, 1 << 30);
    check_space(&roomy, &e.staging, &all).unwrap();
}

// ----------------------------------------------------------- transfer order

#[test]
fn transfer_order_sends_small_config_items_first_cache_and_raw_last() {
    let e = env();
    let fx = fixture(
        &e.home,
        &[
            spec("raw:home/x", SemanticRoot::Home, &[b'r'; 5], 64),
            spec("app/cache", SemanticRoot::Cache, &[b'c'; 3], 64),
            spec("app/big", SemanticRoot::UserConfig, &[b'B'; 300], 100),
            spec("app/small", SemanticRoot::UserData, &[b's'; 10], 64),
            // Second file of app/big shares the small item's chunk.
            spec("app/big", SemanticRoot::UserConfig, &[b's'; 10], 64),
            spec("app/big", SemanticRoot::UserConfig, &[b'b'; 20], 64),
        ],
    );
    let order = transfer_order(&fx.plan, &fx.manifest).unwrap();
    let tags: Vec<String> = order
        .iter()
        .map(|c| {
            let b = fx.bytes(c);
            format!("{}{}@{}", b[0] as char, b.len(), c.offset)
        })
        .collect();
    assert_eq!(
        tags,
        [
            "s10@0", // app/small (10 B)
            "B100@0", "b20@0", // app/big (330 B): file order, then offsets…
            "c3@0",  // cache item (late)
            "r5@0",  // raw item (late, larger)
        ]
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>(),
        "{tags:?}"
    );
    // …identical 'B' pieces are one chunk; the shared 's' chunk went with
    // app/small already.
    assert_eq!(order.len(), fx.layout().unique_chunk_count());
}

// ------------------------------------------------------------------- space

#[test]
fn space_counts_data_once_when_state_dir_shares_the_target_filesystem() {
    let e = env();
    let fx = two_files_sharing_a_chunk(&e.home);
    let needs = space_needs(&e.platform, &fx.plan, &fx.manifest).unwrap();
    assert_eq!(needs.len(), 1, "{needs:?}");
    let n = &needs[0];
    assert!(n.state_dir);
    assert_eq!(n.roots, [SemanticRoot::UserConfig]);
    assert_eq!(n.bytes, 5010);
    assert_eq!(n.needed, required_space(5010));
    assert_eq!(required_space(1000), 1000 + 50 + 64 * 1024 * 1024);
}

#[test]
fn space_counts_staged_and_final_copy_on_different_filesystems() {
    let e = env_with(|p, home| {
        std::fs::create_dir_all(home.join(".config")).unwrap();
        p.filesystem(home.join(".config"), 7, 1234)
            .disk_total_bytes(9999)
    });
    let fx = two_files_sharing_a_chunk(&e.home);
    let needs = space_needs(&e.platform, &fx.plan, &fx.manifest).unwrap();
    assert_eq!(needs.len(), 2, "{needs:?}");
    let (state, target) = (&needs[0], &needs[1]);
    assert!(state.state_dir && state.roots.is_empty());
    assert_eq!(state.bytes, 5010);
    assert!(!target.state_dir);
    assert_eq!(target.roots, [SemanticRoot::UserConfig]);
    assert_eq!(target.bytes, 5010);
    assert_eq!(target.available, Some(1234));
    assert_eq!(target.total, Some(9999));
    assert!(target.is_short() && !state.is_short());
    assert_eq!(target.describe(), "user_config");
}

// -------------------------------------------------------------------- perf

/// Not a benchmark gate: prints staging throughput for the wave notes.
#[test]
fn staging_throughput_64_mib() {
    const SIZE: usize = 64 * 1024 * 1024;
    const PIECE: usize = 64 * 1024;
    let e = env();
    let data: Vec<u8> = (0..SIZE).map(|i| (i % 251) as u8).collect();
    let fx = fixture(
        &e.home,
        &[spec("app/big", SemanticRoot::UserConfig, &data, PIECE)],
    );
    let all: Vec<ChunkRef> = fx.manifest.items[0].entries[0].chunks().to_vec();
    let start = Instant::now();
    let mut store = e.open(fx.layout(), CheckpointPolicy::Periodic);
    for c in &all {
        store.put(&e.platform, c, fx.bytes(c)).unwrap();
    }
    let staged = store.finish(&e.platform).unwrap();
    let secs = start.elapsed().as_secs_f64();
    assert_eq!(staged.files.len(), 1);
    println!(
        "staged 64 MiB ({} chunks, verify + write + checkpoints + whole-file check) in {secs:.3} s = {:.0} MB/s",
        all.len(),
        SIZE as f64 / secs / 1e6
    );
}
