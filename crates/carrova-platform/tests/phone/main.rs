//! Phone data types: every one survives the CBOR wire the link lanes use.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fmt::Debug;

use carrova_platform::phone::{
    AudioSide, CallKind, CallRecord, CallState, Hidden, Note, PermState, PhoneEvent, PhonePerm,
    SendOutcome, SmsMessage, SmsThread,
};
use serde::Serialize;
use serde::de::DeserializeOwned;

fn roundtrip<T: Serialize + DeserializeOwned + PartialEq + Debug>(value: &T) {
    let mut bytes = Vec::new();
    ciborium::into_writer(value, &mut bytes).unwrap();
    let back: T = ciborium::from_reader(bytes.as_slice()).unwrap();
    assert_eq!(&back, value);
}

#[test]
fn phone_types_cbor_roundtrip() {
    let note = Note {
        key: "0|com.chat|7|null|10123".into(),
        app_id: "com.chat".into(),
        app_name: "Chat".into(),
        title: "Ana".into(),
        text: "On my way ✓".into(),
        when_ms: 1_763_300_000_000,
        can_reply: true,
        icon: Some(vec![0x89, b'P', b'N', b'G']),
        hidden: Some(Hidden::Code),
    };
    let message = SmsMessage {
        id: 9,
        thread_id: 3,
        outgoing: false,
        text: "hi".into(),
        when_ms: -1,
    };
    roundtrip(&note);
    roundtrip(&SmsThread {
        id: 3,
        name: None,
        address: "+15550100".into(),
        snippet: "hi".into(),
        when_ms: 1,
        unread: 2,
    });
    roundtrip(&CallRecord {
        number: "+15550100".into(),
        name: Some("Ana".into()),
        when_ms: 1,
        duration_s: 61,
        kind: CallKind::Declined,
    });
    for state in [
        CallState::Idle,
        CallState::Ringing {
            number: "+15550100".into(),
            name: None,
        },
        CallState::Active {
            since_ms: 5,
            muted: true,
            on_laptop_audio: false,
        },
        CallState::Ended { missed: true },
    ] {
        roundtrip(&state);
    }
    for event in [
        PhoneEvent::NotePosted(note),
        PhoneEvent::NoteRemoved { key: "k".into() },
        PhoneEvent::SmsReceived(message),
        PhoneEvent::Call(CallState::Idle),
        PhoneEvent::Dropped { count: 4 },
    ] {
        roundtrip(&event);
    }
    for hidden in [Hidden::Code, Hidden::TextOff, Hidden::Locked] {
        roundtrip(&hidden);
    }
    for outcome in [
        SendOutcome::Sent,
        SendOutcome::NoSignal,
        SendOutcome::Failed,
    ] {
        roundtrip(&outcome);
    }
    roundtrip(&AudioSide::Phone);
    roundtrip(&AudioSide::Laptop);
    for perm in [
        PhonePerm::NotificationAccess,
        PhonePerm::Sms,
        PhonePerm::Calls,
        PhonePerm::Contacts,
        PhonePerm::BluetoothConnect,
    ] {
        roundtrip(&perm);
    }
    for state in [PermState::Granted, PermState::Denied, PermState::Restricted] {
        roundtrip(&state);
    }
}
