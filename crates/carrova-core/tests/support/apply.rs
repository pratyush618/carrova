//! Shared helpers for the `apply_*` integration tests.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use carrova_core::ArtifactId;
use carrova_core::plan::{Action, ObjectKind, Plan, PlanOptions, PlannedAction, PlannedHandler};
use carrova_core::session::SessionId;
use carrova_core::test_support::stage::Staged;
use carrova_manifest::{
    Blake3Hash, Entry, EntryKind, ItemFlags, Manifest, ManifestItem, RelPath, SourceInfo,
};
use carrova_platform::fake::FakePlatform;
use carrova_platform::{FileKind, OsKind, Platform, SemanticRoot};

pub const SID: &str = "sess-1";

pub struct Env {
    pub tmp: tempfile::TempDir,
    pub home: PathBuf,
    pub session_dir: PathBuf,
    pub staging: PathBuf,
    pub platform: FakePlatform,
    pub staged: Staged,
    /// Digest of every staged file, by `(item key, entry index)`.
    pub digests: BTreeMap<(String, usize), (Blake3Hash, u64)>,
}

impl Env {
    pub fn new() -> Self {
        Self::with(|p| p)
    }

    pub fn with(f: impl FnOnce(FakePlatform) -> FakePlatform) -> Self {
        let tmp = tempfile::TempDir::new().unwrap();
        let home = tmp.path().join("home");
        std::fs::create_dir(&home).unwrap();
        let state = tmp.path().join("state");
        let session_dir = state.join("sessions").join(SID);
        std::fs::create_dir_all(&session_dir).unwrap();
        let staging = session_dir.join("staging");
        std::fs::create_dir(&staging).unwrap();
        let platform = f(FakePlatform::with_home(&home).with_state_dir(&state));
        Self {
            tmp,
            home,
            session_dir,
            staging,
            platform,
            staged: Staged::default(),
            digests: BTreeMap::new(),
        }
    }

    pub fn root(&self, r: SemanticRoot) -> PathBuf {
        self.platform.resolve_root(r).unwrap()
    }

    /// Stage `bytes` for `(item, idx)`.
    pub fn stage(&mut self, item: &str, idx: usize, bytes: &[u8]) {
        let p = self
            .staging
            .join(format!("{}-{idx}", item.replace('/', "_")));
        std::fs::write(&p, bytes).unwrap();
        self.platform.set_mode(&p, 0o600).unwrap();
        self.staged.files.insert((item.into(), idx), p);
        self.digests.insert(
            (item.into(), idx),
            (Blake3Hash::of(bytes), bytes.len() as u64),
        );
    }

    /// Manifest the staged files came from: one raw item per item key,
    /// file entries at their staged indices (directories elsewhere).
    pub fn manifest(&self) -> Manifest {
        let mut items: BTreeMap<&str, Vec<Entry>> = BTreeMap::new();
        for ((item, idx), (hash, size)) in &self.digests {
            let entries = items.entry(item).or_default();
            while entries.len() <= *idx {
                entries.push(Entry {
                    root: SemanticRoot::Home,
                    rel: RelPath::new(format!("e{}", entries.len())).unwrap(),
                    kind: EntryKind::Dir,
                    mode: None,
                    mtime_ns: None,
                });
            }
            entries[*idx].kind = EntryKind::File {
                size: *size,
                hash: *hash,
                chunks: Vec::new(),
            };
        }
        Manifest {
            format_version: 2,
            created_at: 0,
            source: SourceInfo {
                os: OsKind::Linux,
                carrova_version: "test".into(),
                device_name: None,
            },
            source_home: None,
            items: items
                .into_iter()
                .map(|(item, entries)| ManifestItem {
                    recipe_id: None,
                    item_id: item.into(),
                    label: item.into(),
                    flags: ItemFlags::default(),
                    entries,
                    handler: None,
                    artifacts: Vec::new(),
                    source_paths: Vec::new(),
                })
                .collect(),
        }
    }

    pub fn stage_artifact(&mut self, item: &str, name: &str, bytes: &[u8]) {
        let p = self
            .staging
            .join(format!("{}-art-{name}", item.replace('/', "_")));
        std::fs::write(&p, bytes).unwrap();
        self.staged.artifacts.insert(
            ArtifactId {
                item: item.into(),
                name: name.into(),
            },
            p,
        );
    }

    /// Write a file with an exact mode (via the platform: umask-independent).
    pub fn put(&self, path: &Path, bytes: &[u8], mode: u32) {
        crate::common::put_at(&self.platform, path, bytes, mode);
    }

    pub fn mode(&self, path: &Path) -> u32 {
        self.platform.metadata(path).unwrap().mode.unwrap() & 0o7777
    }

    pub fn snap(&self) -> Snapshot {
        snapshot(&self.platform, &self.home)
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "mirrors the PlannedAction fields a test spells out inline"
)]
pub fn action(
    env: &Env,
    item: &str,
    idx: usize,
    kind: ObjectKind,
    root: SemanticRoot,
    rel: &str,
    size: u64,
    mode: Option<u32>,
    action: Action,
) -> PlannedAction {
    PlannedAction {
        item: item.into(),
        entry_index: idx,
        kind,
        root,
        rel: rel.into(),
        abs: env.root(root).join(rel),
        size,
        mode,
        action,
        link_target: None,
        mtime_ns: None,
    }
}

/// File action staging `bytes` too.
#[expect(
    clippy::too_many_arguments,
    reason = "mirrors the PlannedAction fields a test spells out inline"
)]
pub fn file(
    env: &mut Env,
    item: &str,
    idx: usize,
    root: SemanticRoot,
    rel: &str,
    bytes: &[u8],
    mode: u32,
    act: Action,
) -> PlannedAction {
    env.stage(item, idx, bytes);
    action(
        env,
        item,
        idx,
        ObjectKind::File,
        root,
        rel,
        bytes.len() as u64,
        Some(mode),
        act,
    )
}

pub fn plan(actions: Vec<PlannedAction>, handlers: Vec<PlannedHandler>) -> Plan {
    let bytes_to_write = actions
        .iter()
        .filter(|a| a.kind == ObjectKind::File)
        .map(|a| a.size)
        .sum();
    Plan {
        session: SessionId(SID.into()),
        options: PlanOptions::default(),
        actions,
        handlers,
        bytes_to_write,
        free_space: None,
        missing_apps: vec![],
        running_apps: vec![],
        app_names: Default::default(),
        mappings: Vec::new(),
        plugins: Vec::new(),
        blockers: vec![],
        warnings: vec![],
        sensitive: Default::default(),
    }
}

/// One node of a directory snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
    File { mode: u32, bytes: Vec<u8> },
    Dir { mode: u32 },
    Symlink { target: PathBuf },
}

pub type Snapshot = BTreeMap<PathBuf, Node>;

/// Paths, kinds, bytes, modes and link targets below `root` (never follows).
pub fn snapshot(p: &dyn Platform, root: &Path) -> Snapshot {
    let mut out = BTreeMap::new();
    walk(p, root, root, &mut out);
    out
}

fn walk(p: &dyn Platform, root: &Path, dir: &Path, out: &mut Snapshot) {
    for entry in p.read_dir(dir).unwrap() {
        let path = dir.join(entry.name);
        let m = p.metadata(&path).unwrap();
        let rel = path.strip_prefix(root).unwrap().to_path_buf();
        let mode = m.mode.unwrap_or(0) & 0o7777;
        match m.kind {
            FileKind::File => {
                out.insert(
                    rel,
                    Node::File {
                        mode,
                        bytes: std::fs::read(&path).unwrap(),
                    },
                );
            }
            FileKind::Dir => {
                out.insert(rel, Node::Dir { mode });
                walk(p, root, &path, out);
            }
            FileKind::Symlink => {
                out.insert(
                    rel,
                    Node::Symlink {
                        target: p.read_link(&path).unwrap(),
                    },
                );
            }
            FileKind::Other => panic!("unexpected special file {}", path.display()),
        }
    }
}

/// A non-trivial existing home: config dir, ssh key, docs.
pub fn seed_home(env: &Env) {
    let h = &env.home;
    std::fs::create_dir_all(h.join(".config/app")).unwrap();
    env.platform
        .set_mode(&h.join(".config/app"), 0o750)
        .unwrap();
    env.put(
        &h.join(".config/app/settings.json"),
        b"{\"old\":true}",
        0o640,
    );
    env.put(&h.join(".ssh/id_ed25519"), b"OLD-PRIVATE-KEY", 0o600);
    env.platform.set_mode(&h.join(".ssh"), 0o700).unwrap();
    env.put(&h.join("notes.txt"), b"old notes", 0o644);
}
