use carrova_manifest::ChunkHash;

use super::*;
use crate::wire::{ChunkRequest, Handshake};

#[test]
fn handshake_byte_fields_encode_as_cbor_byte_strings() {
    let frame = encode_frame(&Handshake::Pake(vec![7u8; 33])).unwrap();
    // 0x58 0x21 = CBOR byte string of 33 bytes (not an integer array).
    assert!(frame.windows(2).any(|w| w == [0x58, 0x21]));
    assert!(frame.len() < 4 + 33 + 16);
}

#[test]
fn oversized_frame_is_refused_on_encode() {
    let big = ChunkRequest {
        hashes: vec![ChunkHash::of(b"x"); MAX_FRAME_LEN / 32 + 1],
    };
    assert!(matches!(
        encode_frame(&big),
        Err(TransportError::FrameTooLarge { .. })
    ));
}
