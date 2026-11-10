//! [`StageLayout`]: where every needed chunk lands inside the staged files
//! of an approved plan.

use std::collections::HashMap;

use carrova_manifest::{Blake3Hash, ChunkHash, ChunkRef, EntryKind, Manifest, ManifestItem};
use carrova_platform::SemanticRoot;

use crate::error::{CoreError, Result};
use crate::key::{ArtifactId, ItemKey};
use crate::plan::{Action, ObjectKind, Plan};

/// Mode of every staged file (owner-only, never looser): staging creates
/// them with it, and apply restores it before reading one back.
pub const STAGED_MODE: u32 = 0o600;

/// Domain separator of [`StageLayout::fingerprint`]; bump when the staged
/// file format or the fingerprint encoding changes.
const FINGERPRINT_DOMAIN: &[u8] = b"carrova/stage-layout/1";

/// What a staged file becomes once applied.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum StageTarget {
    /// Content of file entry `entry_index` of item `item` (a planned
    /// create / replace / keep-both action).
    File {
        /// Item key.
        item: ItemKey,
        /// Entry index within the manifest item.
        entry_index: usize,
    },
    /// A handler artifact.
    Artifact(ArtifactId),
}

impl StageTarget {
    /// Item key this staged file belongs to.
    #[must_use]
    pub fn item(&self) -> &ItemKey {
        match self {
            Self::File { item, .. } | Self::Artifact(ArtifactId { item, .. }) => item,
        }
    }

    /// `item#entry` or `item/artifact`, for messages.
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Self::File { item, entry_index } => format!("{item}#{entry_index}"),
            Self::Artifact(id) => id.to_string(),
        }
    }
}

/// One staged file (`staging/files/<n>`, `n` = its index in
/// [`StageLayout::files`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedFileSpec {
    /// What it becomes.
    pub target: StageTarget,
    /// Planned target root (after remap); `None` for artifacts.
    pub root: Option<SemanticRoot>,
    /// Final size in bytes.
    pub size: u64,
    /// BLAKE3 of the whole file (manifest).
    pub hash: Blake3Hash,
    /// Chunks in offset order, contiguous over `0..size`.
    pub chunks: Vec<ChunkRef>,
}

/// One place a chunk's bytes go: staged file index + byte offset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Place {
    /// Index into [`StageLayout::files`].
    pub file: usize,
    /// Byte offset within that file.
    pub offset: u64,
}

/// Every place one distinct chunk is written to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChunkPlaces {
    pub(crate) len: u32,
    pub(crate) places: Vec<Place>,
}

/// The staged files an approved plan needs and, per distinct chunk, every
/// `(file, offset)` its bytes belong at — a chunk shared by several files
/// (or repeated within one) is received once and written everywhere.
///
/// Only actions that write content are staged: planned `Create`,
/// `Replace` and `KeepBoth` of file entries, plus every artifact of an
/// item with a planned handler. Directories, symlinks, `Skip` and
/// `Identical` need no data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageLayout {
    files: Vec<StagedFileSpec>,
    chunks: HashMap<ChunkHash, ChunkPlaces>,
    /// Distinct chunk hashes in first-seen (file, offset) order.
    order: Vec<ChunkHash>,
    fingerprint: Blake3Hash,
}

impl StageLayout {
    /// Layout of `plan` against `manifest`. A plan that does not match the
    /// manifest (unknown item or entry, kind mismatch) or chunk lists that
    /// do not tile their file are a [`CoreError::Verification`] error.
    pub fn new(plan: &Plan, manifest: &Manifest) -> Result<Self> {
        let items: HashMap<ItemKey, &ManifestItem> =
            manifest.items.iter().map(|i| (ItemKey::of(i), i)).collect();
        let mismatch =
            |what: String| CoreError::Verification(format!("plan does not match manifest: {what}"));

        let mut files = Vec::new();
        for a in &plan.actions {
            if !matches!(
                a.action,
                Action::Create | Action::Replace | Action::KeepBoth { .. }
            ) {
                continue;
            }
            let entry = items
                .get(&a.item)
                .and_then(|i| i.entries.get(a.entry_index))
                .ok_or_else(|| mismatch(format!("{}#{}", a.item, a.entry_index)))?;
            match (&entry.kind, a.kind) {
                (EntryKind::File { size, hash, chunks }, ObjectKind::File) => {
                    files.push(StagedFileSpec {
                        target: StageTarget::File {
                            item: a.item.clone(),
                            entry_index: a.entry_index,
                        },
                        root: Some(a.root),
                        size: *size,
                        hash: *hash,
                        chunks: chunks.clone(),
                    });
                }
                (EntryKind::Dir, ObjectKind::Dir)
                | (EntryKind::Symlink { .. }, ObjectKind::Symlink) => {}
                _ => return Err(mismatch(format!("{}#{} kind", a.item, a.entry_index))),
            }
        }
        for h in &plan.handlers {
            let item = items
                .get(&h.item)
                .ok_or_else(|| mismatch(h.item.to_string()))?;
            for art in &item.artifacts {
                files.push(StagedFileSpec {
                    target: StageTarget::Artifact(ArtifactId {
                        item: h.item.clone(),
                        name: art.name.clone(),
                    }),
                    root: None,
                    size: art.size,
                    hash: art.hash,
                    chunks: art.chunks.clone(),
                });
            }
        }
        Self::from_files(files)
    }

    fn from_files(files: Vec<StagedFileSpec>) -> Result<Self> {
        let mut chunks: HashMap<ChunkHash, ChunkPlaces> = HashMap::new();
        let mut order = Vec::new();
        let mut fp = blake3::Hasher::new();
        fp.update(FINGERPRINT_DOMAIN);
        fp.update(&(files.len() as u64).to_le_bytes());
        for (n, spec) in files.iter().enumerate() {
            let bad =
                |why: &str| CoreError::Verification(format!("{}: {why}", spec.target.label()));
            hash_target(&mut fp, &spec.target);
            fp.update(&spec.size.to_le_bytes());
            fp.update(spec.hash.as_bytes());
            fp.update(&(spec.chunks.len() as u64).to_le_bytes());
            let mut next = 0u64;
            for c in &spec.chunks {
                if c.offset != next || c.len == 0 {
                    return Err(bad("chunks do not tile the file"));
                }
                next = next.saturating_add(u64::from(c.len));
                fp.update(c.hash.as_bytes());
                fp.update(&c.offset.to_le_bytes());
                fp.update(&c.len.to_le_bytes());
                let place = Place {
                    file: n,
                    offset: c.offset,
                };
                match chunks.get_mut(&c.hash) {
                    Some(p) if p.len != c.len => {
                        return Err(bad("one chunk hash listed with two lengths"));
                    }
                    Some(p) => p.places.push(place),
                    None => {
                        order.push(c.hash);
                        chunks.insert(
                            c.hash,
                            ChunkPlaces {
                                len: c.len,
                                places: vec![place],
                            },
                        );
                    }
                }
            }
            if next != spec.size {
                return Err(bad("chunks do not cover the file size"));
            }
        }
        Ok(Self {
            files,
            chunks,
            order,
            fingerprint: fp.finalize().into(),
        })
    }

    /// Staged files; index `n` is stored as `staging/files/<n>`.
    #[must_use]
    pub fn files(&self) -> &[StagedFileSpec] {
        &self.files
    }

    /// Every place the chunk `hash` is written to (empty if not needed).
    #[must_use]
    pub fn places(&self, hash: &ChunkHash) -> &[Place] {
        self.chunks.get(hash).map_or(&[], |p| p.places.as_slice())
    }

    pub(crate) fn chunk(&self, hash: &ChunkHash) -> Option<&ChunkPlaces> {
        self.chunks.get(hash)
    }

    /// The chunk `hash` as a [`ChunkRef`] at its first place, if needed.
    #[must_use]
    pub fn chunk_ref(&self, hash: &ChunkHash) -> Option<ChunkRef> {
        let p = self.chunks.get(hash)?;
        let first = p.places.first()?;
        Some(ChunkRef {
            hash: *hash,
            offset: first.offset,
            len: p.len,
        })
    }

    /// Distinct needed chunks, in first-seen (file, offset) order.
    pub fn unique_chunks(&self) -> impl Iterator<Item = ChunkRef> + '_ {
        self.order.iter().filter_map(|h| self.chunk_ref(h))
    }

    /// Number of distinct needed chunks.
    #[must_use]
    pub fn unique_chunk_count(&self) -> usize {
        self.order.len()
    }

    /// Sum of all staged file sizes (the space staging occupies).
    #[must_use]
    pub fn total_bytes(&self) -> u64 {
        self.files
            .iter()
            .fold(0u64, |acc, f| acc.saturating_add(f.size))
    }

    /// Digest of the whole layout (targets, sizes, hashes, chunk places):
    /// staging written for a different layout is never reused.
    #[must_use]
    pub fn fingerprint(&self) -> Blake3Hash {
        self.fingerprint
    }
}

fn hash_target(fp: &mut blake3::Hasher, target: &StageTarget) {
    let mut text = |s: &str| {
        fp.update(&(s.len() as u64).to_le_bytes());
        fp.update(s.as_bytes());
    };
    match target {
        StageTarget::File { item, entry_index } => {
            text("file");
            text(item.as_str());
            fp.update(&(*entry_index as u64).to_le_bytes());
        }
        StageTarget::Artifact(ArtifactId { item, name }) => {
            text("artifact");
            text(item.as_str());
            text(name);
        }
    }
}
