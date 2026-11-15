//! Public-API tests for ring messages.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[path = "../support/mod.rs"]
mod support;

use std::thread;

use carrova_link::{LaneKind, RING_SECONDS, RingMsg, accept_lane, recv_ring, send_ring};
use support::{Open, T, linked};

/// Send `msg` and return what the receiver got.
fn ring(msg: RingMsg) -> RingMsg {
    let (a, b) = linked();
    thread::scope(|s| {
        let server = s.spawn(|| {
            let (kind, stream) = accept_lane(&b, &Open, T).unwrap();
            assert_eq!(kind, LaneKind::Ring);
            recv_ring(stream).unwrap()
        });
        send_ring(&a, msg).unwrap();
        server.join().unwrap()
    })
}

#[test]
fn ring_start_stop_roundtrip() {
    let start = RingMsg::Start { seconds: 12 };
    assert_eq!(ring(start), start);
    assert_eq!(ring(RingMsg::Stop), RingMsg::Stop);
}

#[test]
fn receiver_clamps_a_long_ring_to_the_default() {
    assert_eq!(
        ring(RingMsg::Start { seconds: u16::MAX }),
        RingMsg::Start {
            seconds: RING_SECONDS
        }
    );
}

#[test]
fn ring_messages_roundtrip_through_serde() {
    for msg in [
        RingMsg::Start {
            seconds: RING_SECONDS,
        },
        RingMsg::Stop,
    ] {
        let json = serde_json::to_string(&msg).unwrap();
        assert_eq!(serde_json::from_str::<RingMsg>(&json).unwrap(), msg);
    }
    assert_eq!(RING_SECONDS, 30);
}
