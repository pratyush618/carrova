use std::time::{Duration, SystemTime, UNIX_EPOCH};

use carrova_platform::{FileKind, Platform, SemanticRoot};

use crate::{ChunkParams, Entry, EntryKind, ManifestError, RelPath, Result, chunk_reader};

/// Build an [`Entry`] for `rel` below `root`.
///
/// The on-disk path is derived here from [`Platform::resolve_root`] and the
/// validated [`RelPath`] components; callers cannot point it elsewhere.
/// Uses lstat semantics via [`Platform::metadata`]: symlinks are recorded with
/// their raw target ([`Platform::read_link`]) and never followed; regular
/// files are read via [`Platform::open_read_nofollow`] (so a file swapped for
/// a symlink after the lstat is refused, not followed) and chunked with
/// `params`; other object types (FIFOs, devices, sockets) are rejected. The
/// mode is masked to `0o777`; setuid, setgid and sticky bits are dropped.
///
/// Ancestor directories between the root and the entry are not re-checked
/// here: callers walk them (the recipe resolver never descends through
/// symlinks).
pub fn entry_from_path(
    platform: &dyn Platform,
    root: SemanticRoot,
    rel: &str,
    params: ChunkParams,
) -> Result<Entry> {
    let rel = RelPath::new(rel)?;
    let mut abs = platform.resolve_root(root)?;
    abs.extend(rel.components());
    let meta = platform.metadata(&abs)?;
    let kind = match meta.kind {
        FileKind::File => {
            let file = platform.open_read_nofollow(&abs)?;
            let fc = chunk_reader(std::io::BufReader::new(file), params).map_err(|e| match e {
                ManifestError::Io(source) => ManifestError::IoAt {
                    path: abs.clone(),
                    source,
                },
                other => other,
            })?;
            EntryKind::File {
                size: fc.size,
                hash: fc.file_hash,
                chunks: fc.chunks,
            }
        }
        FileKind::Dir => EntryKind::Dir,
        FileKind::Symlink => {
            let target = platform.read_link(&abs)?;
            let target = target
                .to_str()
                .ok_or_else(|| ManifestError::NonUtf8Path(target.clone()))?
                .to_owned();
            EntryKind::Symlink { target }
        }
        FileKind::Other => return Err(ManifestError::UnsupportedFileKind(abs)),
    };
    Ok(Entry {
        root,
        rel,
        kind,
        // Permission bits only: setuid/setgid/sticky are never captured.
        mode: meta.mode.map(|m| m & 0o777),
        mtime_ns: meta.modified.and_then(unix_nanos),
    })
}

/// `t` as signed nanoseconds since the epoch (negative before 1970; the
/// encoding of [`Entry::mtime_ns`](crate::Entry::mtime_ns)); `None` if it
/// does not fit in an `i64`.
#[must_use]
pub fn unix_nanos(t: SystemTime) -> Option<i64> {
    match t.duration_since(UNIX_EPOCH) {
        Ok(d) => i64::try_from(d.as_nanos()).ok(),
        Err(e) => i64::try_from(e.duration().as_nanos()).ok().map(|n| -n),
    }
}

/// Inverse of [`unix_nanos`]: `ns` since the epoch as a [`SystemTime`]
/// (the epoch itself if out of range).
#[must_use]
pub fn time_from_unix_nanos(ns: i64) -> SystemTime {
    let d = Duration::from_nanos(ns.unsigned_abs());
    if ns >= 0 {
        UNIX_EPOCH.checked_add(d)
    } else {
        UNIX_EPOCH.checked_sub(d)
    }
    .unwrap_or(UNIX_EPOCH)
}
