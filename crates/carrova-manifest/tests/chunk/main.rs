//! Content-defined chunking: determinism, size bounds, reassembly, edit locality and parameter validation.

#![allow(clippy::unwrap_used)]

use std::collections::HashSet;

use carrova_manifest::{
    Blake3Hash, ChunkParams, DEFAULT_CHUNK_PARAMS, FileChunks, ManifestError, chunk_file,
    chunk_reader,
};

/// Deterministic xorshift64* byte stream.
fn pseudo_random(len: usize, seed: u64) -> Vec<u8> {
    let mut state = seed.max(1);
    let mut out = Vec::with_capacity(len + 8);
    while out.len() < len {
        state ^= state >> 12;
        state ^= state << 25;
        state ^= state >> 27;
        out.extend_from_slice(&state.wrapping_mul(0x2545_F491_4F6C_DD1D).to_le_bytes());
    }
    out.truncate(len);
    out
}

fn chunk(data: &[u8]) -> FileChunks {
    chunk_reader(data, DEFAULT_CHUNK_PARAMS).unwrap()
}

fn reassemble(data: &[u8], fc: &FileChunks) -> Vec<u8> {
    let mut out = Vec::new();
    for c in &fc.chunks {
        let start = usize::try_from(c.offset).unwrap();
        let piece = &data[start..start + c.len as usize];
        assert_eq!(Blake3Hash::of(piece), c.hash);
        out.extend_from_slice(piece);
    }
    out
}

#[test]
fn deterministic() {
    let data = pseudo_random(1 << 20, 7);
    assert_eq!(chunk(&data), chunk(&data));
}

#[test]
fn boundaries_respect_min_max() {
    let p = DEFAULT_CHUNK_PARAMS;
    for data in [pseudo_random(2 << 20, 1), vec![0u8; 1 << 20]] {
        let fc = chunk(&data);
        assert!(fc.chunks.len() > 1);
        let (last, rest) = fc.chunks.split_last().unwrap();
        for c in rest {
            assert!(c.len >= p.min && c.len <= p.max, "len {}", c.len);
        }
        assert!(last.len >= 1 && last.len <= p.max);
    }
}

#[test]
fn reassembly_matches_original() {
    let data = pseudo_random(1_500_000, 3);
    let fc = chunk(&data);
    assert_eq!(fc.size, data.len() as u64);
    assert_eq!(fc.file_hash, Blake3Hash::of(&data));
    let mut expected_offset = 0;
    for c in &fc.chunks {
        assert_eq!(c.offset, expected_offset);
        expected_offset += u64::from(c.len);
    }
    assert_eq!(reassemble(&data, &fc), data);
}

#[test]
fn small_input_is_single_chunk() {
    for len in [1usize, 100, 16 * 1024] {
        let data = pseudo_random(len, 9);
        let fc = chunk(&data);
        assert_eq!(fc.chunks.len(), 1, "len {len}");
        assert_eq!(fc.chunks[0].offset, 0);
        assert_eq!(fc.chunks[0].len as usize, len);
        assert_eq!(fc.chunks[0].hash, fc.file_hash);
    }
}

#[test]
fn empty_input_has_no_chunks() {
    let fc = chunk(&[]);
    assert!(fc.chunks.is_empty());
    assert_eq!(fc.size, 0);
    assert_eq!(fc.file_hash, Blake3Hash::of(b""));
}

#[test]
fn insertion_near_start_keeps_most_chunks() {
    let original = pseudo_random(2 << 20, 42);
    let mut shifted = original.clone();
    let insert = pseudo_random(100, 99);
    shifted.splice(1000..1000, insert);

    let a = chunk(&original);
    let b = chunk(&shifted);
    let b_hashes: HashSet<_> = b.chunks.iter().map(|c| c.hash).collect();
    let shared = a
        .chunks
        .iter()
        .filter(|c| b_hashes.contains(&c.hash))
        .count();
    assert!(a.chunks.len() >= 8);
    assert!(
        shared * 10 >= a.chunks.len() * 8,
        "only {shared}/{} chunks shared",
        a.chunks.len()
    );
}

#[test]
fn invalid_params_rejected_not_panicking() {
    let bad = [
        ChunkParams {
            min: 1,
            avg: 1024,
            max: 4096,
        },
        ChunkParams {
            min: 4096,
            avg: 1024,
            max: 8192,
        },
        ChunkParams {
            min: 1024,
            avg: 4096,
            max: 64 << 20,
        },
    ];
    for p in bad {
        assert!(matches!(
            chunk_reader(&b"abc"[..], p),
            Err(ManifestError::InvalidChunkParams(_))
        ));
    }
}

#[test]
fn chunk_file_reads_from_disk() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("f");
    let data = pseudo_random(300_000, 5);
    std::fs::write(&path, &data).unwrap();
    assert_eq!(
        chunk_file(&path, DEFAULT_CHUNK_PARAMS).unwrap(),
        chunk(&data)
    );
    assert!(matches!(
        chunk_file(&dir.path().join("missing"), DEFAULT_CHUNK_PARAMS),
        Err(ManifestError::IoAt { .. })
    ));
}
