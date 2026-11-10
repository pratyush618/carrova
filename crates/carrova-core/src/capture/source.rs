//! Where each captured chunk's bytes live on this machine
//! ([`CaptureSource`]) and the [`ChunkSource`] that re-reads and
//! re-verifies them.

use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;

use carrova_manifest::{Blake3Hash, ChunkHash, ChunkRef, ChunkSource, ChunkSourceError};
use carrova_platform::{Platform, SemanticRoot};
use serde::{Deserialize, Serialize};

use crate::key::RootedPath;

/// Internal chunk locator: captured files as semantic root + relative path
/// (never absolute paths) and handler artifact bytes.
#[derive(Default, Serialize, Deserialize)]
pub(super) struct CaptureSource {
    /// Captured regular files.
    files: Vec<SourceFile>,
    /// First place each distinct chunk was seen.
    #[serde(with = "location_list")]
    locations: HashMap<ChunkHash, Location>,
    /// Handler artifact bytes (never printed). Persisted as they are:
    /// re-running the handler on resume could produce different bytes
    /// than the manifest promised.
    blobs: Vec<serde_bytes::ByteBuf>,
}

impl std::fmt::Debug for CaptureSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CaptureSource")
            .field("files", &self.files.len())
            .field("chunks", &self.locations.len())
            .field("blobs", &self.blobs.len())
            .finish()
    }
}

/// A captured file, as the manifest names it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct SourceFile {
    root: SemanticRoot,
    rel: carrova_manifest::RelPath,
}

impl SourceFile {
    /// `root:rel`, for error messages.
    fn display(&self) -> String {
        RootedPath::new(self.root, self.rel.as_str()).to_string()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Location {
    File {
        /// Index into [`CaptureSource::files`].
        file: usize,
        offset: u64,
        len: u32,
    },
    Blob {
        /// Index into [`CaptureSource::blobs`].
        index: usize,
        offset: u64,
        len: u32,
    },
}

/// `HashMap<ChunkHash, Location>` as a CBOR list of pairs (keys stay byte
/// strings regardless of the map-key support of the codec).
mod location_list {
    use std::collections::HashMap;

    use carrova_manifest::ChunkHash;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    use super::Location;

    pub(super) fn serialize<S: Serializer>(
        map: &HashMap<ChunkHash, Location>,
        s: S,
    ) -> Result<S::Ok, S::Error> {
        let mut pairs: Vec<(&ChunkHash, &Location)> = map.iter().collect();
        pairs.sort_unstable_by_key(|(h, _)| **h);
        pairs.serialize(s)
    }

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(
        d: D,
    ) -> Result<HashMap<ChunkHash, Location>, D::Error> {
        Ok(Vec::<(ChunkHash, Location)>::deserialize(d)?
            .into_iter()
            .collect())
    }
}

impl CaptureSource {
    pub(super) fn add_file(
        &mut self,
        root: SemanticRoot,
        rel: carrova_manifest::RelPath,
        chunks: &[ChunkRef],
    ) {
        let file = self.files.len();
        self.files.push(SourceFile { root, rel });
        for c in chunks {
            self.locations.entry(c.hash).or_insert(Location::File {
                file,
                offset: c.offset,
                len: c.len,
            });
        }
    }

    pub(super) fn add_blob(&mut self, bytes: Vec<u8>, chunks: &[ChunkRef]) {
        let index = self.blobs.len();
        self.blobs.push(serde_bytes::ByteBuf::from(bytes));
        for c in chunks {
            self.locations.entry(c.hash).or_insert(Location::Blob {
                index,
                offset: c.offset,
                len: c.len,
            });
        }
    }

    /// Every location points at an existing file / blob.
    pub(super) fn is_consistent(&self) -> bool {
        self.locations.values().all(|l| match *l {
            Location::File { file, .. } => file < self.files.len(),
            Location::Blob { index, .. } => index < self.blobs.len(),
        })
    }
}

#[derive(Debug)]
pub(super) struct CaptureReader<'a> {
    source: &'a CaptureSource,
    platform: &'a dyn Platform,
    /// Roots resolved so far.
    roots: HashMap<SemanticRoot, PathBuf>,
}

fn changed(display: &str) -> ChunkSourceError {
    ChunkSourceError::Changed(display.to_owned())
}

impl<'a> CaptureReader<'a> {
    pub(super) fn new(source: &'a CaptureSource, platform: &'a dyn Platform) -> Self {
        Self {
            source,
            platform,
            roots: HashMap::new(),
        }
    }

    fn path_of(&mut self, f: &SourceFile) -> Result<PathBuf, ChunkSourceError> {
        let root = match self.roots.get(&f.root) {
            Some(r) => r.clone(),
            None => {
                let r = self.platform.resolve_root(f.root)?;
                self.roots.insert(f.root, r.clone());
                r
            }
        };
        let mut abs = root;
        abs.extend(f.rel.components());
        Ok(abs)
    }
}

impl ChunkSource for CaptureReader<'_> {
    /// Re-reads the chunk from where it was captured (files re-opened
    /// nofollow) and verifies its BLAKE3 hash: a file that changed since
    /// capture fails loudly instead of writing different data.
    fn read_chunk(&mut self, chunk: &ChunkRef) -> Result<Vec<u8>, ChunkSourceError> {
        let source = self.source;
        let loc = *source
            .locations
            .get(&chunk.hash)
            .ok_or(ChunkSourceError::Missing(chunk.hash))?;
        let to_usize =
            |n: u64| usize::try_from(n).map_err(|_| ChunkSourceError::Missing(chunk.hash));
        let (bytes, display) = match loc {
            Location::File { file, offset, len } => {
                let f = source
                    .files
                    .get(file)
                    .ok_or(ChunkSourceError::Missing(chunk.hash))?;
                let display = f.display();
                if len != chunk.len {
                    return Err(changed(&display));
                }
                let path = self.path_of(f)?;
                let mut h = self.platform.open_read_nofollow(&path)?;
                h.seek(SeekFrom::Start(offset))?;
                let mut buf = vec![0u8; to_usize(u64::from(len))?];
                h.read_exact(&mut buf).map_err(|e| {
                    if e.kind() == std::io::ErrorKind::UnexpectedEof {
                        changed(&display)
                    } else {
                        ChunkSourceError::Io(e)
                    }
                })?;
                (buf, display)
            }
            Location::Blob { index, offset, len } => {
                let start = to_usize(offset)?;
                let end = start.saturating_add(to_usize(u64::from(len))?);
                let buf = source
                    .blobs
                    .get(index)
                    .and_then(|b| b.get(start..end))
                    .ok_or_else(|| changed("handler artifact"))?;
                (buf.to_vec(), "handler artifact".to_owned())
            }
        };
        if Blake3Hash::of(&bytes) != chunk.hash {
            return Err(changed(&display));
        }
        Ok(bytes)
    }
}
