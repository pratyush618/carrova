//! Public-API tests for ring messages.

use carrova_link::{RING_SECONDS, RingMsg};

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
