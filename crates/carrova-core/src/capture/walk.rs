//! Reading the planned items: resolve their paths ([`walk_items`]), then
//! snapshot every file into manifest entries or run the item's handler
//! ([`Capturer`]).

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use carrova_manifest::{
    Artifact, DEFAULT_CHUNK_PARAMS, Entry, EntryKind, chunk_reader, entry_from_path,
};
use carrova_platform::{FileKind, Platform, SemanticRoot};
use carrova_recipe::{FileEntry, resolve_item};
use tracing::debug;

use super::MAX_RECAPTURE;
use super::export::ExportHook;
use super::items::Planned;
use super::source::CaptureSource;
use crate::checklist::Skipped;
use crate::error::{CoreError, Result};
use crate::key::RootedPath;
use crate::secrets::secret_in;
use crate::session::{Event, EventSink, ProgressMeter};

/// The file entries of each planned item (empty for handler items), with
/// exclusions applied and secrets held back ([`held_back`]); a path
/// claimed by two items goes to the first. Held-back paths no item took
/// are reported in `skipped`; resolve problems as warnings.
pub(super) fn walk_items(
    platform: &dyn Platform,
    planned: &[Planned<'_>],
    sink: &dyn EventSink,
    skipped: &mut Vec<Skipped>,
) -> Vec<Vec<FileEntry>> {
    let mut seen: HashSet<(SemanticRoot, String)> = HashSet::new();
    let mut held = Vec::new();
    let resolved = planned
        .iter()
        .map(|p| {
            if p.item.builtin_handler().is_some() {
                return Vec::new();
            }
            let r = resolve_item(&p.item, platform);
            if let Some(first) = r.skipped.first() {
                sink.emit(Event::Warning {
                    message: format!(
                        "{}: {} path(s) left out (e.g. {}:{}: {})",
                        p.key,
                        r.skipped.len(),
                        first.root,
                        first.rel,
                        first.reason
                    ),
                });
            }
            r.entries
                .into_iter()
                .filter(|e| {
                    let path = RootedPath::new(e.root, &e.rel);
                    !p.excluded.iter().any(|ex| path.is_within(*ex))
                })
                // Before claiming: a held-back path stays free for an item that may take it.
                .filter(|e| !(p.unconfirmed_raw && held_back(e, &mut held)))
                .filter(|e| seen.insert((e.root, e.rel.clone())))
                .collect()
        })
        .collect();
    skipped.extend(
        held.into_iter()
            .filter(|path| !seen.contains(path))
            .map(|(root, rel)| Skipped {
                what: RootedPath::new(root, &rel).to_string(),
                reason: HELD_BACK.into(),
            }),
    );
    resolved
}

/// Why an entry of an unconfirmed raw item stays behind.
const HELD_BACK: &str = "looks like a key or password; pick it again to confirm it";

/// Whether an entry of an unconfirmed raw item ([`Planned::unconfirmed_raw`])
/// lies in a secret location ([`secret_in`]) and so stays behind: nothing
/// secret moves without the sender's confirmation. Each location is
/// added to `held` once, at its outermost entry (entries come sorted).
fn held_back(e: &FileEntry, held: &mut Vec<(SemanticRoot, String)>) -> bool {
    if secret_in(&e.rel).is_none() {
        return false;
    }
    let inside_reported = e
        .rel
        .rsplit_once('/')
        .is_some_and(|(parent, _)| secret_in(parent).is_some());
    if !inside_reported {
        held.push((e.root, e.rel.clone()));
    }
    true
}

/// Bytes in the regular files of `resolved`.
pub(super) fn file_bytes(resolved: &[Vec<FileEntry>]) -> u64 {
    resolved
        .iter()
        .flatten()
        .filter(|e| e.kind == FileKind::File)
        .map(|e| e.size)
        .fold(0, u64::saturating_add)
}

/// What one item captured.
pub(super) enum Captured {
    Entries(Vec<Entry>),
    /// Handler name and its artifacts.
    Artifacts(String, Vec<Artifact>),
}

/// Captures items one by one, collecting chunk locations.
pub(super) struct Capturer<'a> {
    platform: &'a dyn Platform,
    sink: &'a dyn EventSink,
    roots: BTreeMap<SemanticRoot, PathBuf>,
    pub(super) source: CaptureSource,
    progress: ProgressMeter<'a>,
    /// Paths left out of otherwise-captured items (e.g. unsafe names).
    pub(super) skipped: Vec<Skipped>,
}

impl<'a> Capturer<'a> {
    pub(super) fn new(
        platform: &'a dyn Platform,
        sink: &'a dyn EventSink,
        progress: ProgressMeter<'a>,
    ) -> Self {
        Self {
            platform,
            sink,
            roots: BTreeMap::new(),
            source: CaptureSource::default(),
            progress,
            skipped: Vec::new(),
        }
    }

    fn root(&mut self, root: SemanticRoot) -> Result<PathBuf> {
        if let Some(p) = self.roots.get(&root) {
            return Ok(p.clone());
        }
        let p = self.platform.resolve_root(root)?;
        self.roots.insert(root, p.clone());
        Ok(p)
    }

    /// Build entries for one item, its files passed through `export` if
    /// given; the item's chunks are registered only if every entry
    /// succeeded.
    pub(super) fn capture_entries(
        &mut self,
        resolved: &[FileEntry],
        mut export: Option<&mut ExportHook<'_>>,
    ) -> Result<Captured> {
        let mut out = Vec::with_capacity(resolved.len());
        // Exported file contents, in file-entry order.
        let mut exported = Vec::new();
        let mut skipped_dirs: Vec<(SemanticRoot, String)> = Vec::new();
        for fe in resolved {
            // Resolved entries are sorted, so a skipped dir precedes its contents:
            // report it once, drop everything below it silently.
            let path = RootedPath::new(fe.root, &fe.rel);
            if skipped_dirs
                .iter()
                .any(|(r, d)| path.is_within(RootedPath::new(*r, d)))
            {
                continue;
            }
            if let Some(why) = unsafe_name(fe) {
                if fe.kind == FileKind::Dir {
                    skipped_dirs.push((fe.root, fe.rel.clone()));
                }
                self.skip_unsafe(fe, why);
                continue;
            }
            let mut abs = self.root(fe.root)?;
            abs.extend(fe.rel.split('/'));
            let mut entry = self.snapshot_file(fe.root, &fe.rel, &abs)?;
            if let EntryKind::File { size, .. } = &entry.kind {
                self.progress.add(*size);
                if let Some(hook) = export.as_deref_mut() {
                    exported.push(hook.apply(self.platform, &abs, &mut entry)?);
                }
            }
            out.push(entry);
        }
        let files = out
            .iter()
            .filter(|e| matches!(e.kind, EntryKind::File { .. }));
        if export.is_some() {
            for (e, bytes) in files.zip(exported) {
                self.source.add_blob(bytes, e.chunks());
            }
        } else {
            for e in files {
                self.source.add_file(e.root, e.rel.clone(), e.chunks());
            }
        }
        Ok(Captured::Entries(out))
    }

    /// Leave out a path whose name could spoof the receiver's display.
    fn skip_unsafe(&mut self, fe: &FileEntry, why: &str) {
        let rel = fe.rel.escape_debug().to_string();
        let what = RootedPath::new(fe.root, &rel).to_string();
        self.sink.emit(Event::Warning {
            message: format!("skipped {what}: {why}"),
        });
        self.skipped.push(Skipped {
            what,
            reason: why.to_owned(),
        });
    }

    /// One entry; regular files are re-read while their size or mtime
    /// changes under us, up to [`MAX_RECAPTURE`] times.
    fn snapshot_file(&self, root: SemanticRoot, rel: &str, abs: &Path) -> Result<Entry> {
        let mut attempt = 0;
        loop {
            let entry = entry_from_path(self.platform, root, rel, DEFAULT_CHUNK_PARAMS)?;
            let EntryKind::File { size, .. } = &entry.kind else {
                return Ok(entry);
            };
            let after = self.platform.metadata(abs)?;
            let stable = after.kind == FileKind::File
                && after.size == *size
                && after.modified.and_then(carrova_manifest::unix_nanos) == entry.mtime_ns;
            if stable {
                return Ok(entry);
            }
            let shown = RootedPath::new(root, rel).to_string();
            if attempt >= MAX_RECAPTURE {
                return Err(CoreError::Unstable(shown));
            }
            attempt += 1;
            debug!(
                path = shown,
                attempt, "file changed while reading; re-capturing"
            );
            self.sink.emit(Event::Recaptured {
                path: shown,
                attempt,
            });
        }
    }

    /// Run handler `name` and chunk its artifacts.
    pub(super) fn run_handler(&mut self, name: &str) -> Result<Captured> {
        let data = crate::handlers::export(self.platform, name)?;
        let mut artifacts = Vec::with_capacity(data.len());
        for d in data {
            let fc = chunk_reader(d.bytes.as_slice(), DEFAULT_CHUNK_PARAMS)?;
            self.source.add_blob(d.bytes, &fc.chunks);
            artifacts.push(Artifact {
                name: d.name,
                size: fc.size,
                hash: fc.file_hash,
                chunks: fc.chunks,
            });
        }
        Ok(Captured::Artifacts(name.to_owned(), artifacts))
    }
}

/// Names (or symlink targets) the manifest rejects because control/bidi
/// characters could spoof the receiver's plan display. Legal on Linux, so
/// such paths are skipped with a reason instead of failing the item.
fn unsafe_name(fe: &FileEntry) -> Option<&'static str> {
    if fe.rel.chars().any(carrova_manifest::is_unsafe_char) {
        return Some("name contains control or bidi characters");
    }
    let target_bad = fe.link_target.as_ref().is_some_and(|t| {
        t.to_string_lossy()
            .chars()
            .any(carrova_manifest::is_unsafe_char)
    });
    target_bad.then_some("symlink target contains control or bidi characters")
}
