use super::*;

#[test]
fn chunk_lengths_above_the_manifest_maximum_are_refused() {
    assert_eq!(chunk_len(MAX_CHUNK_LEN).unwrap(), MAX_CHUNK_LEN as usize);
    assert!(matches!(
        chunk_len(MAX_CHUNK_LEN + 1),
        Err(TransportError::Violation(
            ProtocolViolation::ChunkTooLong { .. }
        ))
    ));
    assert!(matches!(
        chunk_len(u32::MAX),
        Err(TransportError::Violation(
            ProtocolViolation::ChunkTooLong { .. }
        ))
    ));
}

#[test]
fn record_header_round_trips() {
    let h = RecordHeader {
        kind: RecordKind::Data,
        len: 65_536,
        hash: ChunkHash::of(b"chunk"),
    };
    let mut out = Vec::new();
    h.write(&mut out);
    assert_eq!(out.len(), RECORD_HEADER_LEN);
    let mut raw = [0u8; RECORD_HEADER_LEN];
    raw.copy_from_slice(&out);
    assert_eq!(RecordHeader::parse(&raw).unwrap(), h);
}
