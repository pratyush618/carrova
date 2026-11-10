use std::sync::Mutex;

use carrova_manifest::{
    Artifact, ChunkParams, ChunkRef, Entry, EntryKind, ItemFlags, ManifestItem, RelPath,
    SourceInfo, chunk_reader,
};
use carrova_platform::fake::FakePlatform;
use carrova_platform::{FileMeta, OsKind, ProcessInfo, SemanticRoot, SnapshotCapability};
use tempfile::TempDir;

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use carrova_crypto::{
    CryptoError, EncryptWriter, KdfParams, NONCE_PREFIX_LEN, NoncePrefix, Passphrase, SALT_LEN,
    Salt,
};
use carrova_manifest::{
    Blake3Hash, ChunkHash, ChunkSource, ChunkSourceError, DEFAULT_MAX_MANIFEST_BYTES, Manifest,
    ManifestError,
};
use carrova_platform::{Platform, PlatformError};

use super::header::{TAG_CHUNK, TAG_END, TAG_MANIFEST};
use super::write::write_with_kdf;
use super::*;

/// Cheap KDF for tests (bypasses the writer floor via `write_with_kdf`).
const TEST_KDF: KdfParams = KdfParams {
    m_kib: 8 * 1024,
    t: 1,
    p: 1,
};
type Chunk = (ChunkHash, Vec<u8>);

const SMALL_CHUNKS: ChunkParams = ChunkParams {
    min: 64,
    avg: 256,
    max: 1024,
};

// ---------------------------------------------------------------------------
// Test platform: FakePlatform, with std fallbacks for the Phase 2 file
// methods while FakePlatform still returns `Unsupported` for them.

#[derive(Debug)]
struct TestPlatform {
    inner: FakePlatform,
    /// `(mode requested, inner platform honoured it)` per `create_new_file`.
    creates: Mutex<Vec<(Option<u32>, bool)>>,
    fail_rename: bool,
}

impl TestPlatform {
    fn new(home: &Path) -> Self {
        Self {
            inner: FakePlatform::with_home(home),
            creates: Mutex::new(Vec::new()),
            fail_rename: false,
        }
    }
}

fn unsupported<T>(r: &Result<T, PlatformError>) -> bool {
    matches!(r, Err(PlatformError::Unsupported(_)))
}

fn io_err(path: &Path, e: io::Error) -> PlatformError {
    PlatformError::Io {
        path: path.to_path_buf(),
        source: e,
    }
}

impl Platform for TestPlatform {
    fn os(&self) -> OsKind {
        self.inner.os()
    }
    fn resolve_root(&self, root: SemanticRoot) -> Result<PathBuf, PlatformError> {
        self.inner.resolve_root(root)
    }
    fn find_binary(&self, name: &str) -> Result<Option<PathBuf>, PlatformError> {
        self.inner.find_binary(name)
    }
    fn metadata(&self, path: &Path) -> Result<FileMeta, PlatformError> {
        self.inner.metadata(path)
    }
    fn running_processes(&self) -> Result<Vec<ProcessInfo>, PlatformError> {
        self.inner.running_processes()
    }
    fn free_space(&self, path: &Path) -> Result<u64, PlatformError> {
        self.inner.free_space(path)
    }
    fn snapshot_capability(&self, path: &Path) -> Result<SnapshotCapability, PlatformError> {
        self.inner.snapshot_capability(path)
    }

    fn open_read_nofollow(&self, path: &Path) -> Result<File, PlatformError> {
        let r = self.inner.open_read_nofollow(path);
        if !unsupported(&r) {
            return r;
        }
        File::open(path).map_err(|e| io_err(path, e))
    }

    fn create_new_file(&self, path: &Path, mode: Option<u32>) -> Result<File, PlatformError> {
        let r = self.inner.create_new_file(path, mode);
        let native = !unsupported(&r);
        self.creates.lock().unwrap().push((mode, native));
        if native {
            return r;
        }
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|e| io_err(path, e))
    }

    fn rename_noreplace(&self, from: &Path, to: &Path) -> Result<(), PlatformError> {
        if self.fail_rename {
            return Err(io_err(to, io::Error::other("injected rename failure")));
        }
        let r = self.inner.rename_noreplace(from, to);
        if !unsupported(&r) {
            return r;
        }
        // hard_link fails if `to` exists: atomic no-replace for tests.
        std::fs::hard_link(from, to).map_err(|e| io_err(to, e))?;
        std::fs::remove_file(from).map_err(|e| io_err(from, e))
    }

    fn sync_dir(&self, path: &Path) -> Result<(), PlatformError> {
        let r = self.inner.sync_dir(path);
        if !unsupported(&r) {
            return r;
        }
        File::open(path)
            .and_then(|f| f.sync_all())
            .map_err(|e| io_err(path, e))
    }

    fn remove_file(&self, path: &Path) -> Result<(), PlatformError> {
        let r = self.inner.remove_file(path);
        if !unsupported(&r) {
            return r;
        }
        std::fs::remove_file(path).map_err(|e| io_err(path, e))
    }
}

// ---------------------------------------------------------------------------
// Fixtures

fn data(len: usize, seed: u8) -> Vec<u8> {
    let mut x = u32::from(seed) | 1;
    (0..len)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            x as u8
        })
        .collect()
}

#[derive(Default)]
struct MapSource {
    chunks: HashMap<ChunkHash, Vec<u8>>,
    /// Replace bytes of this chunk with garbage.
    corrupt: Option<ChunkHash>,
    /// Fail when asked for this chunk.
    fail: Option<ChunkHash>,
}

impl MapSource {
    fn add(&mut self, content: &[u8]) -> (Blake3Hash, u64, Vec<ChunkRef>) {
        let fc = chunk_reader(content, SMALL_CHUNKS).unwrap();
        for c in &fc.chunks {
            let start = c.offset as usize;
            self.chunks
                .insert(c.hash, content[start..start + c.len as usize].to_vec());
        }
        (fc.file_hash, fc.size, fc.chunks)
    }
}

impl ChunkSource for MapSource {
    fn read_chunk(&mut self, chunk: &ChunkRef) -> Result<Vec<u8>, ChunkSourceError> {
        if self.fail == Some(chunk.hash) {
            return Err(ChunkSourceError::Io(io::Error::other("source failed")));
        }
        let mut b = self.chunks.get(&chunk.hash).cloned().unwrap();
        if self.corrupt == Some(chunk.hash) {
            b[0] ^= 0xff;
        }
        Ok(b)
    }
}

fn file_entry(rel: &str, src: &mut MapSource, content: &[u8]) -> Entry {
    let (hash, size, chunks) = src.add(content);
    Entry {
        root: SemanticRoot::UserConfig,
        rel: RelPath::new(rel).unwrap(),
        kind: EntryKind::File { size, hash, chunks },
        mode: Some(0o600),
        mtime_ns: Some(1),
    }
}

fn item(id: &str, entries: Vec<Entry>, artifacts: Vec<Artifact>) -> ManifestItem {
    ManifestItem {
        recipe_id: Some("app".into()),
        item_id: id.into(),
        label: id.into(),
        flags: ItemFlags::default(),
        entries,
        handler: None,
        artifacts,
        source_paths: Vec::new(),
    }
}

fn manifest(items: Vec<ManifestItem>) -> Manifest {
    Manifest::new(
        1_700_000_000,
        SourceInfo {
            os: OsKind::Linux,
            carrova_version: "test".into(),
            device_name: None,
        },
        items,
    )
}

/// Multi-file manifest with cross-file dedup and a handler artifact.
fn rich_manifest() -> (Manifest, MapSource) {
    let mut src = MapSource::default();
    let a = data(5000, 1);
    let b = data(3000, 2);
    let mut entries = vec![
        file_entry("app/a.conf", &mut src, &a),
        file_entry("app/a-copy.conf", &mut src, &a), // full dedup
        file_entry("app/b.conf", &mut src, &b),
        file_entry("app/empty", &mut src, &[]),
        file_entry("app/tiny", &mut src, b"x"),
    ];
    entries.push(Entry {
        root: SemanticRoot::UserConfig,
        rel: RelPath::new("app/dir").unwrap(),
        kind: EntryKind::Dir,
        mode: Some(0o700),
        mtime_ns: None,
    });
    let art = data(2000, 3);
    let (hash, size, chunks) = src.add(&art);
    let mut it = item(
        "keys",
        vec![file_entry("keys/k", &mut src, &b)], // dedups against b
        vec![Artifact {
            name: "pubring.asc".into(),
            size,
            hash,
            chunks,
        }],
    );
    it.handler = Some("gpg-keyring".into());
    let m = manifest(vec![item("main", entries, vec![]), it]);
    (m, src)
}

struct Env {
    _tmp: TempDir,
    dir: PathBuf,
    platform: TestPlatform,
}

fn env() -> Env {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().join("out");
    std::fs::create_dir(&dir).unwrap();
    let platform = TestPlatform::new(tmp.path());
    Env {
        _tmp: tmp,
        dir,
        platform,
    }
}

fn pass() -> Passphrase {
    Passphrase::new("correct horse battery".into())
}

fn write(env: &Env, name: &str, m: &Manifest, src: &mut MapSource) -> Result<ArchiveSummary> {
    write_with_kdf(
        &env.platform,
        &env.dir.join(name),
        &pass(),
        m,
        src,
        TEST_KDF,
    )
}

/// Open and drain; returns manifest and chunks.
fn read_all(env: &Env, path: &Path, pass: &Passphrase) -> Result<(Manifest, Vec<Chunk>)> {
    let mut r = ArchiveReader::open(&env.platform, path, pass)?;
    let mut chunks = Vec::new();
    while let Some(c) = r.next_chunk()? {
        chunks.push(c);
    }
    Ok((r.manifest().clone(), chunks))
}

fn dir_names(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

// ---------------------------------------------------------------------------
// Round trips

#[test]
fn round_trip_multi_file_dedup_artifacts() {
    let env = env();
    let (m, mut src) = rich_manifest();
    let unique: Vec<ChunkHash> = m.unique_chunk_refs().map(|c| c.hash).collect();
    assert!(
        unique.len() < m.chunk_count(),
        "fixture must exercise dedup"
    );

    let summary = write(&env, "a.carrova", &m, &mut src).unwrap();
    let path = env.dir.join("a.carrova");
    assert_eq!(summary.chunks, unique.len() as u64);
    assert_eq!(summary.bytes, std::fs::metadata(&path).unwrap().len());
    assert_eq!(dir_names(&env.dir), ["a.carrova"]);

    let (got_m, chunks) = read_all(&env, &path, &pass()).unwrap();
    assert_eq!(got_m, m);
    let order: Vec<ChunkHash> = chunks.iter().map(|(h, _)| *h).collect();
    assert_eq!(order, unique, "chunks arrive in unique_chunk_refs order");
    for (h, bytes) in &chunks {
        assert_eq!(&src.chunks[h], bytes);
    }

    let r = ArchiveReader::open(&env.platform, &path, &pass()).unwrap();
    assert_eq!(r.header().version, ARCHIVE_VERSION);
    assert_eq!(r.header().flags, 0);
    assert_eq!(r.header().kdf, TEST_KDF);
}

#[test]
fn round_trip_empty_manifest() {
    let env = env();
    let m = manifest(vec![]);
    let summary = write(&env, "e.carrova", &m, &mut MapSource::default()).unwrap();
    assert_eq!(summary.chunks, 0);
    let (got, chunks) = read_all(&env, &env.dir.join("e.carrova"), &pass()).unwrap();
    assert_eq!(got, m);
    assert!(chunks.is_empty());
}

#[test]
fn chunks_iterator_and_done_is_sticky() {
    let env = env();
    let (m, mut src) = rich_manifest();
    write(&env, "a.carrova", &m, &mut src).unwrap();
    let mut r = ArchiveReader::open(&env.platform, &env.dir.join("a.carrova"), &pass()).unwrap();
    let n = r.chunks().map(Result::unwrap).count();
    assert_eq!(n, m.unique_chunk_refs().count());
    assert!(r.next_chunk().unwrap().is_none());
}

#[test]
fn header_bytes_round_trip() {
    let h = ArchiveHeader {
        version: ARCHIVE_VERSION,
        flags: 0,
        kdf: KdfParams {
            m_kib: 0x0102_0304,
            t: 5,
            p: 2,
        },
        salt: Salt([0xaa; SALT_LEN]),
        nonce: NoncePrefix([0xbb; NONCE_PREFIX_LEN]),
    };
    let b = h.to_bytes();
    assert_eq!(&b[..8], &MAGIC);
    assert_eq!(&b[8..10], &[1, 0]); // little-endian version
    assert_eq!(&b[12..16], &[4, 3, 2, 1]); // little-endian m_kib
    assert_eq!(ArchiveHeader::from_bytes(&b).unwrap(), h);
}

// ---------------------------------------------------------------------------
// Authentication / tampering

#[test]
fn wrong_passphrase() {
    let env = env();
    let (m, mut src) = rich_manifest();
    write(&env, "a.carrova", &m, &mut src).unwrap();
    let r = ArchiveReader::open(
        &env.platform,
        &env.dir.join("a.carrova"),
        &Passphrase::new("wrong".into()),
    );
    assert!(
        matches!(r, Err(ArchiveError::Crypto(CryptoError::Decrypt))),
        "{r:?}"
    );
}

fn tampered(env: &Env, at: usize, xor: u8) -> PathBuf {
    let orig = std::fs::read(env.dir.join("a.carrova")).unwrap();
    let mut b = orig;
    b[at] ^= xor;
    let p = env.dir.join(format!("t-{at}-{xor}.carrova"));
    std::fs::write(&p, b).unwrap();
    p
}

#[test]
fn tampered_header_fields() {
    let env = env();
    let (m, mut src) = rich_manifest();
    write(&env, "a.carrova", &m, &mut src).unwrap();
    let open = |p: &Path| read_all(&env, p, &pass()).map(|_| ());

    // magic
    for at in 0..8 {
        assert!(matches!(
            open(&tampered(&env, at, 0x01)),
            Err(ArchiveError::BadMagic)
        ));
    }
    // version (LE u16 at 8)
    assert!(matches!(
        open(&tampered(&env, 8, 0x02)),
        Err(ArchiveError::UnsupportedVersion(3))
    ));
    assert!(matches!(
        open(&tampered(&env, 9, 0x01)),
        Err(ArchiveError::UnsupportedVersion(0x0101))
    ));
    // flags (LE u16 at 10)
    assert!(matches!(
        open(&tampered(&env, 10, 0x01)),
        Err(ArchiveError::UnsupportedFlags(1))
    ));
    assert!(matches!(
        open(&tampered(&env, 11, 0x80)),
        Err(ArchiveError::UnsupportedFlags(0x8000))
    ));
    // KDF params: small changes stay valid but derive another key.
    for (at, xor) in [(12, 0x01), (16, 0x02), (20, 0x02)] {
        let r = open(&tampered(&env, at, xor));
        assert!(
            matches!(r, Err(ArchiveError::Crypto(CryptoError::Decrypt))),
            "{at}: {r:?}"
        );
    }
    // KDF params out of bounds are refused before derivation.
    for (at, xor) in [(16, 0x01), (20, 0x01), (15, 0x80), (19, 0x80), (23, 0x80)] {
        let r = open(&tampered(&env, at, xor));
        assert!(
            matches!(r, Err(ArchiveError::Crypto(CryptoError::InvalidParams(_)))),
            "{at}: {r:?}"
        );
    }
    // salt and nonce prefix bytes.
    for at in [24, 24 + SALT_LEN - 1, 24 + SALT_LEN, HEADER_LEN - 1] {
        let r = open(&tampered(&env, at, 0x01));
        assert!(
            matches!(r, Err(ArchiveError::Crypto(CryptoError::Decrypt))),
            "{at}: {r:?}"
        );
    }
}

#[test]
fn tampered_body_byte() {
    let env = env();
    let (m, mut src) = rich_manifest();
    write(&env, "a.carrova", &m, &mut src).unwrap();
    let len = std::fs::metadata(env.dir.join("a.carrova")).unwrap().len() as usize;
    for at in [HEADER_LEN, HEADER_LEN + 100, len - 20, len - 1] {
        let r = read_all(&env, &tampered(&env, at, 0x04), &pass());
        assert!(
            matches!(r, Err(ArchiveError::Crypto(CryptoError::Decrypt))),
            "{at}: {r:?}"
        );
    }
}

#[test]
fn bad_magic_and_short_files() {
    let env = env();
    let p = env.dir.join("x");
    std::fs::write(&p, b"hello world, definitely not an archive at all.......").unwrap();
    assert!(matches!(
        read_all(&env, &p, &pass()),
        Err(ArchiveError::BadMagic)
    ));
    std::fs::write(&p, b"").unwrap();
    assert!(matches!(
        read_all(&env, &p, &pass()),
        Err(ArchiveError::BadMagic)
    ));
    std::fs::write(&p, &MAGIC[..4]).unwrap();
    assert!(matches!(
        read_all(&env, &p, &pass()),
        Err(ArchiveError::BadMagic)
    ));
    std::fs::write(&p, [&MAGIC[..], &[1, 0, 0]].concat()).unwrap();
    assert!(matches!(
        read_all(&env, &p, &pass()),
        Err(ArchiveError::Malformed(_))
    ));
}

#[test]
fn truncated_file() {
    let env = env();
    // Enough data for several STREAM segments.
    let mut src = MapSource::default();
    let big = data(200 * 1024, 9);
    let m = manifest(vec![item(
        "big",
        vec![file_entry("big.bin", &mut src, &big)],
        vec![],
    )]);
    write(&env, "a.carrova", &m, &mut src).unwrap();
    let full = std::fs::read(env.dir.join("a.carrova")).unwrap();
    let seg = carrova_crypto::SEGMENT_LEN + carrova_crypto::TAG_LEN;
    let p = env.dir.join("cut.carrova");
    for keep in [
        HEADER_LEN - 1,
        HEADER_LEN,
        HEADER_LEN + 10,
        HEADER_LEN + seg,
        HEADER_LEN + 2 * seg,
        HEADER_LEN + 2 * seg + 1,
        full.len() - 1,
    ] {
        std::fs::write(&p, &full[..keep]).unwrap();
        let r = read_all(&env, &p, &pass());
        assert!(
            matches!(
                r,
                Err(ArchiveError::Malformed(_)
                    | ArchiveError::Crypto(CryptoError::Truncated | CryptoError::Decrypt))
            ),
            "keep {keep}: {r:?}"
        );
        if keep == HEADER_LEN + seg || keep == HEADER_LEN + 2 * seg || keep == HEADER_LEN {
            assert!(
                matches!(r, Err(ArchiveError::Crypto(CryptoError::Truncated))),
                "keep {keep}: {r:?}"
            );
        }
    }
    // Trailing bytes after a valid archive.
    std::fs::write(&p, [&full[..], b"x"].concat()).unwrap();
    assert!(matches!(
        read_all(&env, &p, &pass()),
        Err(ArchiveError::Crypto(CryptoError::Decrypt))
    ));
}

// ---------------------------------------------------------------------------
// Crafted bodies (valid encryption, bad structure)

enum Rec<'a> {
    Manifest(&'a [u8]),
    Chunk(ChunkHash, u32, &'a [u8]),
    End(u64),
    Raw(&'a [u8]),
}

fn craft(env: &Env, name: &str, recs: &[Rec<'_>]) -> PathBuf {
    let header = ArchiveHeader {
        version: ARCHIVE_VERSION,
        flags: 0,
        kdf: TEST_KDF,
        salt: Salt::random().unwrap(),
        nonce: NoncePrefix::random().unwrap(),
    };
    let key = carrova_crypto::derive_key(&pass(), &header.salt, TEST_KDF).unwrap();
    let hb = header.to_bytes();
    let mut out = hb.to_vec();
    let mut w = EncryptWriter::new(&mut out, &key, &header.nonce, &hb).unwrap();
    for r in recs {
        match r {
            Rec::Manifest(b) => {
                w.write_all(&[TAG_MANIFEST]).unwrap();
                w.write_all(&(b.len() as u64).to_le_bytes()).unwrap();
                w.write_all(b).unwrap();
            }
            Rec::Chunk(h, len, b) => {
                w.write_all(&[TAG_CHUNK]).unwrap();
                w.write_all(h.as_bytes()).unwrap();
                w.write_all(&len.to_le_bytes()).unwrap();
                w.write_all(b).unwrap();
            }
            Rec::End(n) => {
                w.write_all(&[TAG_END]).unwrap();
                w.write_all(&n.to_le_bytes()).unwrap();
            }
            Rec::Raw(b) => w.write_all(b).unwrap(),
        }
    }
    w.finish().unwrap();
    let p = env.dir.join(name);
    std::fs::write(&p, out).unwrap();
    p
}

/// Two single-chunk files (each no larger than `SMALL_CHUNKS.min`).
fn two_chunk_fixture() -> (Manifest, Vec<u8>, [Chunk; 2]) {
    let a = data(60, 5);
    let b = data(50, 6);
    let mut src = MapSource::default();
    let m = manifest(vec![item(
        "i",
        vec![file_entry("a", &mut src, &a), file_entry("b", &mut src, &b)],
        vec![],
    )]);
    assert_eq!(m.chunk_count(), 2);
    let cbor = m.to_cbor_vec().unwrap();
    (m, cbor, [(Blake3Hash::of(&a), a), (Blake3Hash::of(&b), b)])
}

fn chunk(c: &Chunk) -> Rec<'_> {
    Rec::Chunk(c.0, c.1.len() as u32, &c.1)
}

fn expect_malformed(env: &Env, p: &Path, needle: &str) {
    match read_all(env, p, &pass()) {
        Err(ArchiveError::Malformed(msg)) => assert!(msg.contains(needle), "{msg}"),
        other => panic!("expected Malformed({needle}), got {other:?}"),
    }
}

#[test]
fn crafted_valid_archive_reads() {
    let env = env();
    let (_, cbor, [a, b]) = two_chunk_fixture();
    let p = craft(
        &env,
        "ok",
        &[Rec::Manifest(&cbor), chunk(&b), chunk(&a), Rec::End(2)],
    );
    // Any order is accepted as long as every chunk arrives exactly once.
    assert_eq!(read_all(&env, &p, &pass()).unwrap().1.len(), 2);
}

#[test]
fn unreferenced_chunk_rejected() {
    let env = env();
    let (_, cbor, [a, b]) = two_chunk_fixture();
    let stray = data(40, 7);
    let p = craft(
        &env,
        "stray",
        &[
            Rec::Manifest(&cbor),
            chunk(&a),
            Rec::Chunk(Blake3Hash::of(&stray), 40, &stray),
            chunk(&b),
            Rec::End(3),
        ],
    );
    expect_malformed(&env, &p, "not referenced");
}

#[test]
fn duplicate_chunk_rejected() {
    let env = env();
    let (_, cbor, [a, b]) = two_chunk_fixture();
    let p = craft(
        &env,
        "dup",
        &[
            Rec::Manifest(&cbor),
            chunk(&a),
            chunk(&a),
            chunk(&b),
            Rec::End(3),
        ],
    );
    expect_malformed(&env, &p, "duplicate");
}

#[test]
fn missing_chunk_rejected() {
    let env = env();
    let (_, cbor, [a, _]) = two_chunk_fixture();
    let p = craft(
        &env,
        "missing",
        &[Rec::Manifest(&cbor), chunk(&a), Rec::End(1)],
    );
    assert!(matches!(
        read_all(&env, &p, &pass()),
        Err(ArchiveError::MissingChunks(1))
    ));
}

#[test]
fn wrong_end_count_rejected() {
    let env = env();
    let (_, cbor, [a, b]) = two_chunk_fixture();
    let p = craft(
        &env,
        "count",
        &[Rec::Manifest(&cbor), chunk(&a), chunk(&b), Rec::End(5)],
    );
    expect_malformed(&env, &p, "END record");
}

#[test]
fn missing_end_record_rejected() {
    let env = env();
    let (_, cbor, [a, b]) = two_chunk_fixture();
    let p = craft(&env, "noend", &[Rec::Manifest(&cbor), chunk(&a), chunk(&b)]);
    expect_malformed(&env, &p, "unexpected end");
}

#[test]
fn data_after_end_rejected() {
    let env = env();
    let (_, cbor, [a, b]) = two_chunk_fixture();
    let p = craft(
        &env,
        "after",
        &[
            Rec::Manifest(&cbor),
            chunk(&a),
            chunk(&b),
            Rec::End(2),
            Rec::Raw(b"z"),
        ],
    );
    expect_malformed(&env, &p, "after END");
}

#[test]
fn chunk_length_checks() {
    let env = env();
    let (_, cbor, [a, b]) = two_chunk_fixture();
    // Oversize length: rejected before reading/allocating the payload.
    let p = craft(
        &env,
        "big",
        &[
            Rec::Manifest(&cbor),
            Rec::Chunk(a.0, MAX_CHUNK_LEN + 1, &[]),
        ],
    );
    expect_malformed(&env, &p, "invalid length");
    let p = craft(
        &env,
        "zero",
        &[Rec::Manifest(&cbor), Rec::Chunk(a.0, 0, &[])],
    );
    expect_malformed(&env, &p, "invalid length");
    // Length differing from the manifest.
    let p = craft(
        &env,
        "len",
        &[
            Rec::Manifest(&cbor),
            Rec::Chunk(a.0, a.1.len() as u32 - 1, &a.1[1..]),
            chunk(&b),
        ],
    );
    expect_malformed(&env, &p, "length differs");
}

#[test]
fn chunk_hash_mismatch_rejected() {
    let env = env();
    let (_, cbor, [a, _]) = two_chunk_fixture();
    let mut bad = a.1.clone();
    bad[0] ^= 1;
    let p = craft(
        &env,
        "hash",
        &[
            Rec::Manifest(&cbor),
            Rec::Chunk(a.0, bad.len() as u32, &bad),
        ],
    );
    assert!(matches!(
        read_all(&env, &p, &pass()),
        Err(ArchiveError::ChunkHashMismatch(h)) if h == a.0
    ));
}

#[test]
fn bad_record_structure_rejected() {
    let env = env();
    let (_, cbor, _) = two_chunk_fixture();
    let p = craft(&env, "tag", &[Rec::Manifest(&cbor), Rec::Raw(&[9])]);
    expect_malformed(&env, &p, "unknown record tag");
    let p = craft(&env, "first", &[Rec::End(0)]);
    expect_malformed(&env, &p, "first record");
    let p = craft(&env, "two", &[Rec::Manifest(&cbor), Rec::Manifest(&cbor)]);
    expect_malformed(&env, &p, "second manifest");
    // Manifest record with trailing bytes inside its declared length.
    let padded = [&cbor[..], &[0u8; 3]].concat();
    let p = craft(&env, "pad", &[Rec::Manifest(&padded), Rec::End(0)]);
    expect_malformed(&env, &p, "trailing bytes");
    // Declared manifest length above the limit.
    let mut huge = vec![TAG_MANIFEST];
    huge.extend_from_slice(&(DEFAULT_MAX_MANIFEST_BYTES + 1).to_le_bytes());
    let p = craft(&env, "huge", &[Rec::Raw(&huge)]);
    assert!(matches!(
        read_all(&env, &p, &pass()),
        Err(ArchiveError::Manifest(ManifestError::TooLarge { .. }))
    ));
    // Garbage CBOR.
    let p = craft(
        &env,
        "cbor",
        &[Rec::Manifest(b"\xff\x00garbage"), Rec::End(0)],
    );
    assert!(read_all(&env, &p, &pass()).is_err());
}

#[test]
fn reader_poisoned_after_error() {
    let env = env();
    let (_, cbor, [a, _]) = two_chunk_fixture();
    let stray = data(40, 7);
    let p = craft(
        &env,
        "stray",
        &[
            Rec::Manifest(&cbor),
            Rec::Chunk(Blake3Hash::of(&stray), 40, &stray),
            chunk(&a),
        ],
    );
    let mut r = ArchiveReader::open(&env.platform, &p, &pass()).unwrap();
    assert!(r.next_chunk().is_err());
    assert!(r.next_chunk().is_err());
    let mut r = ArchiveReader::open(&env.platform, &p, &pass()).unwrap();
    let items: Vec<_> = r.chunks().collect();
    assert_eq!(items.len(), 1);
    assert!(items[0].is_err());
}

// ---------------------------------------------------------------------------
// Writer filesystem behaviour

#[test]
fn existing_target_path_refused() {
    let env = env();
    let (m, mut src) = rich_manifest();
    let path = env.dir.join("a.carrova");
    std::fs::write(&path, b"keep me").unwrap();
    let r = write(&env, "a.carrova", &m, &mut src);
    assert!(
        matches!(&r, Err(ArchiveError::Io(e)) if e.kind() == io::ErrorKind::AlreadyExists),
        "{r:?}"
    );
    assert_eq!(std::fs::read(&path).unwrap(), b"keep me");
    assert_eq!(dir_names(&env.dir), ["a.carrova"]);
}

#[test]
fn temp_removed_on_failure() {
    let env = env();
    let (m, src0) = rich_manifest();
    let target = m.unique_chunk_refs().nth(2).unwrap().hash;

    // Source error mid-stream.
    let mut src = MapSource {
        chunks: src0.chunks.clone(),
        fail: Some(target),
        ..MapSource::default()
    };
    assert!(write(&env, "a.carrova", &m, &mut src).is_err());
    assert!(dir_names(&env.dir).is_empty());

    // Source returns bytes that do not match the hash.
    let mut src = MapSource {
        chunks: src0.chunks.clone(),
        corrupt: Some(target),
        ..MapSource::default()
    };
    assert!(matches!(
        write(&env, "a.carrova", &m, &mut src),
        Err(ArchiveError::ChunkHashMismatch(h)) if h == target
    ));
    assert!(dir_names(&env.dir).is_empty());

    // Failure at the final rename.
    let mut env = env;
    env.platform.fail_rename = true;
    let mut src = MapSource {
        chunks: src0.chunks,
        ..MapSource::default()
    };
    assert!(write(&env, "a.carrova", &m, &mut src).is_err());
    assert!(dir_names(&env.dir).is_empty());
}

#[test]
fn archive_created_with_mode_600() {
    let env = env();
    let (m, mut src) = rich_manifest();
    write(&env, "a.carrova", &m, &mut src).unwrap();
    let creates = env.platform.creates.lock().unwrap().clone();
    assert_eq!(creates.len(), 1);
    let (mode, native) = creates[0];
    assert_eq!(mode, Some(0o600), "writer must request mode 600");
    // FakePlatform now implements create_new_file: check the result too
    // (Unix only: Windows files have no mode bits).
    if native && cfg!(unix) {
        let meta = env.platform.metadata(&env.dir.join("a.carrova")).unwrap();
        assert_eq!(meta.mode.map(|m| m & 0o777), Some(0o600));
    }
}

#[test]
fn write_archive_enforces_kdf_floor() {
    let env = env();
    let (m, mut src) = rich_manifest();
    let r = write_archive(
        &env.platform,
        &env.dir.join("a.carrova"),
        &pass(),
        &m,
        &mut src,
        WriteOptions { kdf: TEST_KDF },
    );
    assert!(matches!(
        r,
        Err(ArchiveError::Crypto(CryptoError::InvalidParams(_)))
    ));
    assert!(dir_names(&env.dir).is_empty());
    assert_eq!(WriteOptions::default().kdf, KdfParams::DEFAULT);
}

#[test]
fn oversize_manifest_chunk_rejected_by_writer() {
    let env = env();
    let mut src = MapSource::default();
    let content = data(10, 1);
    let mut e = file_entry("f", &mut src, &content);
    if let EntryKind::File { chunks, size, .. } = &mut e.kind {
        chunks[0].len = MAX_CHUNK_LEN + 1;
        *size = u64::from(MAX_CHUNK_LEN) + 1;
    }
    let m = manifest(vec![item("i", vec![e], vec![])]);
    assert!(write(&env, "a.carrova", &m, &mut src).is_err());
    assert!(dir_names(&env.dir).is_empty());
}

#[test]
fn errors_do_not_leak_passphrase() {
    let env = env();
    let (m, mut src) = rich_manifest();
    write(&env, "a.carrova", &m, &mut src).unwrap();
    let secret = "s3cr3t-passphrase-value";
    let err = ArchiveReader::open(
        &env.platform,
        &env.dir.join("a.carrova"),
        &Passphrase::new(secret.into()),
    )
    .unwrap_err();
    let text = format!("{err} {err:?}");
    assert!(!text.contains(secret));
}
