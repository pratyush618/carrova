//! `progress.log` encoding (format in the [`super::staging`] module docs):
//! a header naming the layout fingerprint, then one checksummed record of
//! chunk hashes per checkpoint.

use carrova_manifest::{Blake3Hash, ChunkHash};

const MAGIC: &[u8; 8] = b"CARROVSP";
const LOG_VERSION: u16 = 1;
const SUM_LEN: usize = 8;
const HASH_LEN: usize = 32;
pub(super) const HEADER_LEN: usize = MAGIC.len() + 2 + 2 + 4 + HASH_LEN + SUM_LEN;
/// `progress.log` bytes per recorded chunk.
pub const LOG_BYTES_PER_CHUNK: u64 = HASH_LEN as u64;

/// First [`SUM_LEN`] bytes of BLAKE3 over `bytes`.
fn sum(bytes: &[u8]) -> [u8; SUM_LEN] {
    let mut out = [0u8; SUM_LEN];
    out.copy_from_slice(&blake3::hash(bytes).as_bytes()[..SUM_LEN]);
    out
}

/// Header for a layout with `fingerprint`.
pub(super) fn header(fingerprint: Blake3Hash) -> Vec<u8> {
    let mut h = Vec::with_capacity(HEADER_LEN);
    h.extend_from_slice(MAGIC);
    h.extend_from_slice(&LOG_VERSION.to_le_bytes());
    h.extend_from_slice(&0u16.to_le_bytes());
    h.extend_from_slice(&0u32.to_le_bytes());
    h.extend_from_slice(fingerprint.as_bytes());
    let s = sum(&h);
    h.extend_from_slice(&s);
    h
}

/// `bytes` start with the header this build writes for `fingerprint`.
pub(super) fn header_matches(bytes: &[u8], fingerprint: Blake3Hash) -> bool {
    bytes.get(..HEADER_LEN) == Some(header(fingerprint).as_slice())
}

/// One checkpoint record listing `hashes`.
pub(super) fn record(hashes: &[ChunkHash]) -> Vec<u8> {
    let mut r = Vec::with_capacity(4 + hashes.len() * HASH_LEN + SUM_LEN);
    r.extend_from_slice(
        &u32::try_from(hashes.len())
            .unwrap_or(u32::MAX)
            .to_le_bytes(),
    );
    for h in hashes {
        r.extend_from_slice(h.as_bytes());
    }
    let s = sum(&r);
    r.extend_from_slice(&s);
    r
}

/// Hashes of every intact record and the byte length they span; parsing
/// stops at the first truncated or garbled record (torn tail).
pub(super) fn parse_records(bytes: &[u8]) -> (Vec<ChunkHash>, usize) {
    let mut out = Vec::new();
    let mut valid = 0usize;
    while let Some(frame) = next_frame(&bytes[valid..]) {
        let body = &frame[4..frame.len() - SUM_LEN];
        out.extend(
            body.as_chunks::<HASH_LEN>()
                .0
                .iter()
                .map(|h| Blake3Hash(*h)),
        );
        valid += frame.len();
    }
    (out, valid)
}

/// The intact record at the start of `bytes`, if any.
fn next_frame(bytes: &[u8]) -> Option<&[u8]> {
    let count: [u8; 4] = bytes.get(..4)?.try_into().ok()?;
    let frame_len = (u32::from_le_bytes(count) as usize)
        .checked_mul(HASH_LEN)?
        .checked_add(4 + SUM_LEN)?;
    let frame = bytes.get(..frame_len)?;
    let (body, s) = frame.split_at(frame_len - SUM_LEN);
    (sum(body) == s).then_some(frame)
}
