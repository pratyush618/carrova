//! Capture (sender side): turn a [`Selection`] into a [`Manifest`] plus a
//! [`ChunkSource`] that re-reads chunk bytes for the archive writer or the
//! LAN chunk server.
//!
//! Files are read via `Platform::open_read_nofollow`. File-change safety
//! (snapshots deferred): after hashing a file, its metadata (size, mtime)
//! is re-checked; if it changed, the file is re-captured, up to
//! [`MAX_RECAPTURE`] attempts, else the item fails with
//! [`crate::CoreError::Unstable`]. When the archive writer later re-reads a
//! chunk, its hash is verified again.
//!
//! One item failing (unstable file, unreadable path, handler error) never
//! stops the others: it lands in [`Capture::failed`] with its reason.

mod export;
mod items;
mod persist;
mod source;
mod walk;

use carrova_manifest::{ChunkSource, Entry, Manifest, ManifestItem, SourceInfo};
use carrova_platform::{Platform, SemanticRoot};
use carrova_plugin::{ContextField, Plugin};
use carrova_recipe::{FileEntry, HandlerRef, PluginId, Recipe};
use tracing::{debug, warn};

use self::export::Exports;
use self::items::{Planned, plan_items, warn_running};
use self::source::{CaptureReader, CaptureSource};
use self::walk::{Captured, Capturer, file_bytes, walk_items};
use crate::cancel::Cancel;
use crate::checklist::Skipped;
use crate::error::Result;
use crate::select::Selection;
use crate::session::{Event, EventSink, Phase, ProgressMeter};
use crate::time;

/// Re-capture attempts for a file that changes while being read.
pub const MAX_RECAPTURE: u32 = 3;

/// Sender manifest in a session dir ([`Capture::persist`]).
pub const MANIFEST_FILE: &str = "manifest.cbor";
/// Sender chunk locations in a session dir ([`Capture::persist`]).
pub const CAPTURE_FILE: &str = "capture.cbor";

/// Captured manifest + where each chunk's bytes live on this machine.
#[derive(Debug)]
pub struct Capture {
    pub manifest: Manifest,
    /// Items that could not be captured, with reasons (shown in checklist).
    pub failed: Vec<Skipped>,
    source: CaptureSource,
}

impl Capture {
    /// Chunk reader for [`carrova_archive::write_archive`] and the LAN server.
    pub fn chunk_source<'a>(&'a self, platform: &'a dyn Platform) -> impl ChunkSource + 'a {
        CaptureReader::new(&self.source, platform)
    }

    /// The manifest as CBOR (deterministic: a loaded capture encodes to
    /// the bytes it was persisted with), for sending and fingerprinting.
    pub fn manifest_cbor(&self) -> Result<Vec<u8>> {
        Ok(self.manifest.to_cbor_vec()?)
    }
}

/// Capture every included item of `selection` (resolving recipe items and
/// raw dirs fresh, honouring per-path exclusions). Runs built-in handlers
/// for handler items (artifacts). Fails if `selection.issues()` is non-empty.
///
/// `cancel` is checked before each item ([`crate::CoreError::Cancelled`]).
pub fn capture(
    platform: &dyn Platform,
    recipes: &[Recipe],
    selection: &Selection,
    sink: &dyn EventSink,
    cancel: &Cancel,
) -> Result<Capture> {
    capture_with_plugins(platform, recipes, &[], selection, sink, cancel)
}

/// [`capture`] with the `plugins` loaded here: the sender's home is
/// recorded ([`Manifest::source_home`]) only when a captured item's plugin
/// declares [`ContextField::SourceHome`]. No plugin runs here: an item
/// whose plugin declares `export` is left out (see
/// [`capture_with_consent`]).
pub fn capture_with_plugins(
    platform: &dyn Platform,
    recipes: &[Recipe],
    plugins: &[Plugin],
    selection: &Selection,
    sink: &dyn EventSink,
    cancel: &Cancel,
) -> Result<Capture> {
    capture_with_consent(platform, recipes, plugins, &[], selection, sink, cancel)
}

/// [`capture_with_plugins`], also running the sender-side export
/// (`export_transform`) of the plugins in `allowed_exports` — those the
/// user allowed, after seeing their
/// [`permissions`](carrova_plugin::PluginManifest::permissions), to run on
/// this computer. An item whose plugin exports but is not allowed is left
/// out with the reason ([`Capture::failed`]).
pub fn capture_with_consent(
    platform: &dyn Platform,
    recipes: &[Recipe],
    plugins: &[Plugin],
    allowed_exports: &[PluginId],
    selection: &Selection,
    sink: &dyn EventSink,
    cancel: &Cancel,
) -> Result<Capture> {
    selection.check()?;
    let (planned, mut failed) = plan_items(selection, recipes, platform.os());
    warn_running(platform, &planned, sink);
    // Resolve everything first so progress has a byte total.
    let resolved = walk_items(platform, &planned, sink);
    let progress = ProgressMeter::new(
        sink,
        Phase::Capture,
        file_bytes(&resolved),
        ProgressMeter::EVERY_UPDATE,
    )
    // A file may have grown since resolve.
    .estimated()
    .start();
    let mut capturer = Capturer::new(platform, sink, progress);
    let needs_home = planned.iter().any(|p| {
        let Some(HandlerRef::Plugin(id)) = &p.item.handler else {
            return false;
        };
        plugins.iter().any(|plugin| {
            let m = plugin.manifest();
            m.id == *id && m.context.contains(&ContextField::SourceHome)
        })
    });
    let mut exports = Exports::new(plugins, allowed_exports);
    let mut items = Vec::with_capacity(planned.len());
    for (p, entries) in planned.into_iter().zip(resolved) {
        cancel.check()?;
        items.extend(capture_item(
            &mut capturer,
            &mut exports,
            p,
            &entries,
            &mut failed,
            sink,
        ));
    }
    let mut manifest = Manifest::new(
        i64::try_from(time::now_unix()).unwrap_or(i64::MAX),
        SourceInfo {
            os: platform.os(),
            carrova_version: env!("CARGO_PKG_VERSION").into(),
            device_name: crate::about::device_name(platform),
        },
        items,
    );
    // A non-UTF-8 home is left out.
    manifest.source_home = needs_home
        .then(|| platform.resolve_root(SemanticRoot::Home).ok())
        .flatten()
        .and_then(|h| h.to_str().map(str::to_owned));
    manifest.validate()?;
    failed.append(&mut capturer.skipped);
    debug!(
        items = manifest.items.len(),
        failed = failed.len(),
        "capture done"
    );
    Ok(Capture {
        manifest,
        failed,
        source: capturer.source,
    })
}

/// Capture one planned item: its manifest item, or `None` with the reason
/// pushed to `failed`.
fn capture_item(
    capturer: &mut Capturer<'_>,
    exports: &mut Exports<'_>,
    p: Planned<'_>,
    entries: &[FileEntry],
    failed: &mut Vec<Skipped>,
    sink: &dyn EventSink,
) -> Option<ManifestItem> {
    // Plugin items move their files (exported first if the plugin
    // exports); the receiver runs the plugin's transform.
    let result = match p.item.builtin_handler() {
        Some(h) => capturer.run_handler(h.as_str()),
        None => exports
            .hook_for(&p.item)
            .and_then(|hook| capturer.capture_entries(entries, hook)),
    };
    let mut mi = p.manifest_item;
    let (files, bytes) = match result {
        Ok(Captured::Entries(e)) if e.is_empty() => {
            failed.push(Skipped {
                what: p.key.to_string(),
                reason: "nothing to capture (paths missing or excluded)".into(),
            });
            return None;
        }
        Ok(Captured::Entries(e)) => {
            let files = e.iter().filter(|e| e.file_size().is_some()).count();
            let bytes = e.iter().filter_map(Entry::file_size).sum();
            mi.entries = e;
            (files, bytes)
        }
        Ok(Captured::Artifacts(name, a)) => {
            let counts = (a.len(), a.iter().map(|a| a.size).sum());
            mi.handler = Some(name);
            mi.artifacts = a;
            counts
        }
        Err(e) => {
            warn!(item = %p.key, error = %e, "item capture failed");
            failed.push(Skipped {
                what: p.key.to_string(),
                reason: e.to_string(),
            });
            return None;
        }
    };
    sink.emit(Event::ItemCaptured {
        item: p.key.to_owned(),
        files: u64::try_from(files).unwrap_or(u64::MAX),
        bytes,
    });
    Some(mi)
}
