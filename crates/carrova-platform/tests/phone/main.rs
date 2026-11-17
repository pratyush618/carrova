//! Phone data types: every one survives the CBOR wire the link lanes use.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fmt::Debug;

use carrova_platform::phone::{
    AudioSide, CallKind, CallRecord, CallState, Cursor, Hidden, Note, PermState, PhoneEvent,
    PhonePerm, SendOutcome, SmsMessage, SmsThread,
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
        id: 12,
        number: "+15550100".into(),
        name: Some("Ana".into()),
        when_ms: 1,
        duration_s: 61,
        kind: CallKind::Declined,
    });
    roundtrip(&Cursor { when_ms: 5, id: -2 });
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
        PhoneEvent::SmsStored { thread_id: 7 },
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
        SendOutcome::Unknown,
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
        PhonePerm::Battery,
    ] {
        roundtrip(&perm);
    }
    for state in [PermState::Granted, PermState::Denied, PermState::Restricted] {
        roundtrip(&state);
    }
}

#[test]
fn debug_output_hides_phone_content() {
    let s = || "SECRET-123".to_string();
    let note = Note {
        key: s(),
        app_id: s(),
        app_name: s(),
        title: s(),
        text: s(),
        when_ms: 1,
        can_reply: false,
        icon: None,
        hidden: None,
    };
    let shown = [
        format!("{note:?}"),
        format!(
            "{:?}",
            SmsThread {
                id: 1,
                name: Some(s()),
                address: s(),
                snippet: s(),
                when_ms: 1,
                unread: 0,
            }
        ),
        format!(
            "{:?}",
            SmsMessage {
                id: 1,
                thread_id: 1,
                outgoing: true,
                text: s(),
                when_ms: 1,
            }
        ),
        format!(
            "{:?}",
            CallState::Ringing {
                number: s(),
                name: Some(s()),
            }
        ),
        format!(
            "{:?}",
            CallRecord {
                id: 1,
                number: s(),
                name: Some(s()),
                when_ms: 1,
                duration_s: 1,
                kind: CallKind::Missed,
            }
        ),
        format!("{:?}", PhoneEvent::NotePosted(note)),
        format!("{:?}", PhoneEvent::NoteRemoved { key: s() }),
    ];
    for out in shown {
        assert!(!out.contains("SECRET"), "{out}");
    }
}

#[test]
fn cursor_orders_by_time_then_id() {
    let c = |when_ms, id| Cursor { when_ms, id };
    assert!(c(1, 9) < c(2, 0));
    assert!(c(5, 1) < c(5, 2));
    assert_eq!(c(5, 1), c(5, 1));
}

#[test]
fn icon_is_a_cbor_byte_string() {
    let mut note = Note {
        key: String::new(),
        app_id: String::new(),
        app_name: String::new(),
        title: String::new(),
        text: String::new(),
        when_ms: 0,
        can_reply: false,
        icon: Some(vec![0xAB; 1000]),
        hidden: None,
    };
    let mut bytes = Vec::new();
    ciborium::into_writer(&note, &mut bytes).unwrap();
    assert!(bytes.len() < 1100, "{} bytes", bytes.len());
    note.icon = None;
    bytes.clear();
    ciborium::into_writer(&note, &mut bytes).unwrap();
    let back: Note = ciborium::from_reader(bytes.as_slice()).unwrap();
    assert_eq!(back, note);
}
