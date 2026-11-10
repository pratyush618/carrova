//! Structural validation of untrusted manifests ([`Manifest::validate`]).

use std::collections::{HashMap, HashSet};

use carrova_platform::SemanticRoot;

use crate::model::{
    MAX_ARTIFACTS_PER_ITEM, MAX_DEVICE_NAME, MAX_ENTRIES, MAX_ITEMS, MAX_SOURCE_PATHS_PER_ITEM,
};
use crate::text::check_text;
use crate::{
    Artifact, Blake3Hash, ChunkFault, ChunkRef, Entry, EntryKind, InvalidReason, MAX_CHUNK_LEN,
    Manifest, ManifestError, ManifestItem, Result,
};

impl Manifest {
    /// Structural checks a receiver relies on (paths are already checked by
    /// [`RelPath`](crate::RelPath), the source OS by its type): supported
    /// version, count limits ([`MAX_ITEMS`], [`MAX_ENTRIES`],
    /// [`MAX_ARTIFACTS_PER_ITEM`], [`MAX_SOURCE_PATHS_PER_ITEM`]), non-empty
    /// ids, no control / bidi characters in any displayed string (ids, label,
    /// handler, artifact names, symlink targets, source version, home and
    /// device name, the latter at most [`MAX_DEVICE_NAME`] characters),
    /// mode bits, symlink targets, and chunk lists contiguous and covering
    /// each file exactly with chunks of `1..=`[`MAX_CHUNK_LEN`] bytes.
    ///
    /// Across all items it also rejects duplicate `(root, rel)` entries,
    /// any entry lying below a `Symlink` or `File` entry of the same root
    /// (which would make the receiver write through it) and a chunk hash
    /// listed with more than one length.
    ///
    /// Roots are only resolved on the receiver, so aliasing *between* roots
    /// (e.g. `home:.config/x` vs `user_config:x`) cannot be detected here;
    /// the receiver's apply step must still refuse to write through symlinks.
    pub fn validate(&self) -> Result<()> {
        crate::wire::check_version(self.format_version)?;
        self.check_limits()?;
        check_text("source carrova_version", &self.source.carrova_version)?;
        if let Some(name) = &self.source.device_name {
            check_text("source device_name", name)?;
            check_count(
                "source device_name characters",
                name.chars().count(),
                MAX_DEVICE_NAME,
            )?;
        }
        if let Some(home) = &self.source_home {
            check_text("source_home", home)?;
        }
        let mut index = EntryIndex::default();
        for item in &self.items {
            item.validate()?;
            for entry in &item.entries {
                entry.validate()?;
                index.insert(entry)?;
            }
        }
        index.check_ancestors()?;
        self.check_chunk_lengths()
    }

    fn check_limits(&self) -> Result<()> {
        check_count("items", self.items.len(), MAX_ITEMS)?;
        let entries = self
            .items
            .iter()
            .fold(0usize, |n, i| n.saturating_add(i.entries.len()));
        check_count("entries", entries, MAX_ENTRIES)
    }

    /// One length per chunk hash, so a receiver can size buffers by hash.
    fn check_chunk_lengths(&self) -> Result<()> {
        let mut lens: HashMap<&Blake3Hash, u32> = HashMap::new();
        for c in self.all_chunk_refs() {
            if *lens.entry(&c.hash).or_insert(c.len) != c.len {
                return Err(InvalidReason::ConflictingChunkLength { hash: c.hash }.into());
            }
        }
        Ok(())
    }
}

impl ManifestItem {
    /// Item fields and artifacts. Entries are checked by
    /// [`Manifest::validate`] as it indexes them.
    fn validate(&self) -> Result<()> {
        self.check_text_fields()?;
        let item = || self.item_id.clone();
        if self.item_id.is_empty() {
            return Err(InvalidReason::EmptyItemId.into());
        }
        if self.recipe_id.as_deref() == Some("") {
            return Err(InvalidReason::EmptyRecipeId { item: item() }.into());
        }
        if self.handler.as_deref() == Some("") {
            return Err(InvalidReason::EmptyHandler { item: item() }.into());
        }
        if !self.artifacts.is_empty() && self.handler.is_none() {
            return Err(InvalidReason::ArtifactsWithoutHandler { item: item() }.into());
        }
        let mut names = HashSet::new();
        for artifact in &self.artifacts {
            if artifact.name.is_empty() || !names.insert(artifact.name.as_str()) {
                return Err(InvalidReason::BadArtifactName { item: item() }.into());
            }
            artifact.validate()?;
        }
        Ok(())
    }

    /// Text first: later error messages quote these fields.
    fn check_text_fields(&self) -> Result<()> {
        check_text("item_id", &self.item_id)?;
        if let Some(r) = &self.recipe_id {
            check_text("recipe_id", r)?;
        }
        check_text("label", &self.label)?;
        if let Some(h) = &self.handler {
            check_text("handler", h)?;
        }
        check_count(
            "artifacts in one item",
            self.artifacts.len(),
            MAX_ARTIFACTS_PER_ITEM,
        )?;
        check_count(
            "source paths in one item",
            self.source_paths.len(),
            MAX_SOURCE_PATHS_PER_ITEM,
        )?;
        self.artifacts
            .iter()
            .try_for_each(|a| check_text("artifact name", &a.name))
    }
}

impl Artifact {
    fn validate(&self) -> Result<()> {
        check_chunks(self.size, &self.chunks).map_err(|fault| InvalidReason::ArtifactChunks {
            name: self.name.clone(),
            fault,
        })?;
        Ok(())
    }
}

impl Entry {
    fn validate(&self) -> Result<()> {
        if let Some(mode) = self.mode.filter(|m| m & !0o777 != 0) {
            return Err(ManifestError::UnsafeMode {
                root: self.root,
                path: self.rel.to_string(),
                mode,
            });
        }
        match &self.kind {
            EntryKind::File { size, chunks, .. } => {
                check_chunks(*size, chunks).map_err(|fault| InvalidReason::EntryChunks {
                    root: self.root,
                    path: self.rel.clone(),
                    fault,
                })?;
            }
            EntryKind::Dir => {}
            EntryKind::Symlink { target } => {
                if target.is_empty() {
                    return Err(InvalidReason::EmptySymlinkTarget {
                        root: self.root,
                        path: self.rel.clone(),
                    }
                    .into());
                }
                // Also rejects NUL.
                check_text("symlink target", target)?;
            }
        }
        Ok(())
    }
}

/// Every entry's kind by `(root, rel)`, across all items.
#[derive(Default)]
struct EntryIndex<'m> {
    kinds: HashMap<(SemanticRoot, &'m str), &'m EntryKind>,
    order: Vec<&'m Entry>,
}

impl<'m> EntryIndex<'m> {
    /// Add `entry`, rejecting a second entry at the same path.
    fn insert(&mut self, entry: &'m Entry) -> Result<()> {
        if self
            .kinds
            .insert((entry.root, entry.rel.as_str()), &entry.kind)
            .is_some()
        {
            return Err(ManifestError::DuplicateEntry {
                root: entry.root,
                path: entry.rel.to_string(),
            });
        }
        self.order.push(entry);
        Ok(())
    }

    /// No entry may lie below a symlink or file entry of the same root.
    fn check_ancestors(&self) -> Result<()> {
        for entry in &self.order {
            let path = entry.rel.as_str();
            for (i, _) in path.match_indices('/') {
                let ancestor = &path[..i];
                if matches!(
                    self.kinds.get(&(entry.root, ancestor)),
                    Some(EntryKind::Symlink { .. } | EntryKind::File { .. })
                ) {
                    return Err(ManifestError::NonDirAncestor {
                        root: entry.root,
                        path: path.to_owned(),
                        ancestor: ancestor.to_owned(),
                    });
                }
            }
        }
        Ok(())
    }
}

fn check_count(what: &'static str, count: usize, limit: usize) -> Result<()> {
    if count > limit {
        return Err(ManifestError::TooMany { what, count, limit });
    }
    Ok(())
}

/// Chunks must be `1..=MAX_CHUNK_LEN` long, contiguous and cover exactly
/// `0..size`.
fn check_chunks(size: u64, chunks: &[ChunkRef]) -> std::result::Result<(), ChunkFault> {
    let mut next = 0u64;
    for c in chunks {
        if !(1..=MAX_CHUNK_LEN).contains(&c.len) {
            return Err(ChunkFault::LengthOutOfRange { len: c.len });
        }
        if c.offset != next {
            return Err(ChunkFault::NotContiguous);
        }
        next = next
            .checked_add(u64::from(c.len))
            .ok_or(ChunkFault::OffsetOverflow)?;
    }
    if next != size {
        return Err(ChunkFault::SizeMismatch);
    }
    Ok(())
}
