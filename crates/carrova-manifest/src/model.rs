use std::collections::HashSet;

use carrova_platform::SemanticRoot;
use serde::{Deserialize, Serialize};

use crate::{Blake3Hash, ChunkRef, RelPath};

/// Current manifest wire-format version written by this build.
pub const FORMAT_VERSION: u32 = 2;

/// Most items a manifest may hold (checked by [`Manifest::validate`]).
pub const MAX_ITEMS: usize = 100_000;
/// Most entries a manifest may hold across all items.
pub const MAX_ENTRIES: usize = 1_000_000;
/// Most artifacts a single item may hold.
pub const MAX_ARTIFACTS_PER_ITEM: usize = 1024;

/// OS-neutral description of everything a transfer moves.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub format_version: u32,
    /// Creation time, seconds since the Unix epoch.
    pub created_at: i64,
    pub source: SourceInfo,
    pub items: Vec<ManifestItem>,
}

/// Where a manifest was produced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceInfo {
    /// Free-form OS description (e.g. `"linux"`).
    pub os: String,
    pub carrova_version: String,
}

/// One user-selectable unit (a recipe item, or a raw unrecognised path).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestItem {
    /// Recipe id; `None` for raw / unrecognised items.
    pub recipe_id: Option<String>,
    pub item_id: String,
    pub label: String,
    pub flags: ItemFlags,
    pub entries: Vec<Entry>,
    /// Built-in handler (recipe schema v2) that produced `artifacts` and
    /// applies them on the receiver, e.g. `gpg-keyring`. Format v2+.
    #[serde(default)]
    pub handler: Option<String>,
    /// Opaque handler output (e.g. an exported keyring). Format v2+.
    #[serde(default)]
    pub artifacts: Vec<Artifact>,
}

/// Opaque handler output, content-addressed like a file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Artifact {
    /// Handler-defined name, unique within the item (e.g. `pubring.asc`).
    pub name: String,
    pub size: u64,
    /// BLAKE3 of the whole artifact.
    pub hash: Blake3Hash,
    /// Contiguous chunks covering `0..size`; empty iff `size == 0`.
    pub chunks: Vec<ChunkRef>,
}

/// Item properties the receiver needs. Mirrors recipe flags without
/// depending on `carrova-recipe`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ItemFlags {
    /// Contains secrets (keys, tokens); needs explicit confirmation.
    pub sensitive: bool,
    /// The owning app must be closed while applying.
    pub requires_closed: bool,
    pub portability: Portability,
}

/// Whether an item can move between OS families.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Portability {
    #[default]
    SameOs,
    CrossOs,
}

/// One filesystem object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub root: SemanticRoot,
    pub rel: RelPath,
    pub kind: EntryKind,
    /// Permission bits, where the source OS has them. Only `0o777` bits are
    /// allowed: setuid, setgid and sticky are rejected by validation (user
    /// data never needs them, and restoring them would be a privilege risk).
    pub mode: Option<u32>,
    /// Modification time, nanoseconds since the Unix epoch.
    pub mtime_ns: Option<i64>,
}

/// Type-specific entry data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryKind {
    File {
        size: u64,
        /// BLAKE3 of the whole file.
        hash: Blake3Hash,
        /// Contiguous chunks covering `0..size`; empty iff `size == 0`.
        chunks: Vec<ChunkRef>,
    },
    Dir,
    Symlink {
        /// Link target exactly as stored (never followed or resolved).
        target: String,
    },
}

impl Manifest {
    /// New manifest at the current [`FORMAT_VERSION`].
    #[must_use]
    pub fn new(created_at: i64, source: SourceInfo, items: Vec<ManifestItem>) -> Self {
        Self {
            format_version: FORMAT_VERSION,
            created_at,
            source,
            items,
        }
    }

    /// All entries across all items.
    pub fn entries(&self) -> impl Iterator<Item = &Entry> {
        self.items.iter().flat_map(|i| i.entries.iter())
    }

    /// Sum of file sizes (bytes before dedup).
    #[must_use]
    pub fn total_size(&self) -> u64 {
        self.entries()
            .filter_map(Entry::file_size)
            .fold(0, u64::saturating_add)
    }

    /// Number of chunk references (before dedup).
    #[must_use]
    pub fn chunk_count(&self) -> usize {
        self.entries().map(|e| e.chunks().len()).sum()
    }

    /// Each distinct chunk once, with the first entry that contains it
    /// (enough to locate its bytes: `entry.root`/`entry.rel` + `chunk.offset`).
    pub fn unique_chunks(&self) -> impl Iterator<Item = (&Entry, &ChunkRef)> {
        let mut seen = HashSet::new();
        self.entries()
            .flat_map(|e| e.chunks().iter().map(move |c| (e, c)))
            .filter(move |(_, c)| seen.insert(c.hash))
    }

    /// Every distinct chunk (entries and artifacts), each once, in stable
    /// order: entries first, then artifacts. This is the archive chunk order.
    pub fn unique_chunk_refs(&self) -> impl Iterator<Item = &ChunkRef> {
        let mut seen = HashSet::new();
        self.all_chunk_refs().filter(move |c| seen.insert(c.hash))
    }

    /// Every chunk reference (entries, then artifacts), duplicates included.
    pub(crate) fn all_chunk_refs(&self) -> impl Iterator<Item = &ChunkRef> {
        self.entries().flat_map(|e| e.chunks().iter()).chain(
            self.items
                .iter()
                .flat_map(|i| i.artifacts.iter().flat_map(|a| a.chunks.iter())),
        )
    }

    /// Bytes that actually need to move after chunk dedup.
    #[must_use]
    pub fn unique_size(&self) -> u64 {
        self.unique_chunks()
            .map(|(_, c)| u64::from(c.len))
            .fold(0, u64::saturating_add)
    }
}

impl Entry {
    /// File size, or `None` for non-files.
    #[must_use]
    pub fn file_size(&self) -> Option<u64> {
        match self.kind {
            EntryKind::File { size, .. } => Some(size),
            _ => None,
        }
    }

    /// Chunk list (empty for non-files).
    #[must_use]
    pub fn chunks(&self) -> &[ChunkRef] {
        match &self.kind {
            EntryKind::File { chunks, .. } => chunks,
            _ => &[],
        }
    }
}
