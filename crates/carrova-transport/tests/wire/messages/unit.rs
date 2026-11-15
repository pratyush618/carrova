use carrova_platform::OsKind;
use serde::Serialize;

use super::*;

fn encode<T: Serialize>(value: &T) -> Vec<u8> {
    let mut out = Vec::new();
    ciborium::into_writer(value, &mut out).expect("encode");
    out
}

fn decode(bytes: &[u8]) -> Handshake {
    ciborium::from_reader(bytes).expect("decode")
}

/// A `Hello` from before `purpose` existed.
#[derive(Serialize)]
enum OldHandshake {
    Hello {
        proto: u32,
        intent: Intent,
        identify: bool,
    },
}

#[test]
fn old_hello_decodes_as_move() {
    let old = OldHandshake::Hello {
        proto: 1,
        intent: Intent::Pair { nameplate: 7 },
        identify: true,
    };
    let Handshake::Hello {
        intent, purpose, ..
    } = decode(&encode(&old))
    else {
        panic!("not a hello");
    };
    assert_eq!(intent, Intent::Pair { nameplate: 7 });
    assert_eq!(purpose, PairPurpose::Move);
}

#[test]
fn pair_only_hello_round_trips() {
    let hello = Handshake::Hello {
        proto: 1,
        intent: Intent::Pair { nameplate: 7 },
        identify: true,
        purpose: PairPurpose::PairOnly,
    };
    let Handshake::Hello { purpose, .. } = decode(&encode(&hello)) else {
        panic!("not a hello");
    };
    assert_eq!(purpose, PairPurpose::PairOnly);
}

#[test]
fn link_intent_roundtrip() {
    let hello = Handshake::Hello {
        proto: 1,
        intent: Intent::Link {
            nonce: [1; 16],
            tag: [2; 16],
        },
        identify: false,
        purpose: PairPurpose::Move,
    };
    let Handshake::Hello { intent, .. } = decode(&encode(&hello)) else {
        panic!("not a hello");
    };
    assert_eq!(
        intent,
        Intent::Link {
            nonce: [1; 16],
            tag: [2; 16]
        }
    );
}

#[test]
fn linked_roundtrip() {
    assert!(matches!(
        decode(&encode(&Handshake::Linked)),
        Handshake::Linked
    ));
}

#[test]
fn peer_info_roundtrip() {
    let info = Handshake::PeerInfo {
        name: "laptop".to_owned(),
        os: OsKind::Android,
    };
    let Handshake::PeerInfo { name, os } = decode(&encode(&info)) else {
        panic!("not peer info");
    };
    assert_eq!((name.as_str(), os), ("laptop", OsKind::Android));
}
