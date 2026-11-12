use carrova_manifest::ChunkHash;

use super::*;
use crate::wire::{ChunkRequest, Handshake, Intent};

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

/// `Hello { proto: 1, intent: Pair { nameplate: 42 } }` byte for byte as
/// receivers encoded it before `identify` existed.
const OLD_HELLO: &[u8] =
    b"\xa1\x65Hello\xa2\x65proto\x01\x66intent\xa1\x64Pair\xa1\x69nameplate\x18\x2a";

/// `Handshake` as senders decoded it before `identify` existed.
#[derive(serde::Deserialize)]
enum OldHandshake {
    Hello { proto: u32, intent: Intent },
}

#[test]
fn hello_from_an_old_receiver_asks_for_no_identity() {
    let Handshake::Hello {
        proto,
        intent,
        identify,
    } = decode_body(OLD_HELLO).unwrap()
    else {
        panic!("not a hello");
    };
    assert_eq!(proto, 1);
    assert_eq!(intent, Intent::Pair { nameplate: 42 });
    assert!(!identify);
}

#[test]
fn identifying_hello_still_reads_on_an_old_sender() {
    let frame = encode_frame(&Handshake::Hello {
        proto: 1,
        intent: Intent::Pair { nameplate: 42 },
        identify: true,
    })
    .unwrap();
    let OldHandshake::Hello { proto, intent } = decode_body(&frame[FRAME_HEADER_LEN..]).unwrap();
    assert_eq!(proto, 1);
    assert_eq!(intent, Intent::Pair { nameplate: 42 });
}
