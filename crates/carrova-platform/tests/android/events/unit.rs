use super::*;
use crate::phone::{CallKind, CallRecord, CallState, Note, SmsMessage};

fn event(json: &str) -> PhoneEvent {
    match parse(json) {
        Ok(Some(event)) => event,
        other => panic!("{json}: {other:?}"),
    }
}

#[test]
fn parses_each_event_type() {
    let note = event(
        r#"{"type":"note_posted","key":"k1","app_id":"org.app","app_name":"App",
            "title":"Hi","text":"there","when_ms":5,"can_reply":true,"icon":[137,80]}"#,
    );
    assert_eq!(
        note,
        PhoneEvent::NotePosted(Note {
            key: "k1".into(),
            app_id: "org.app".into(),
            app_name: "App".into(),
            title: "Hi".into(),
            text: "there".into(),
            when_ms: 5,
            can_reply: true,
            icon: Some(vec![137, 80]),
            hidden: None,
        })
    );
    let PhoneEvent::NotePosted(no_icon) = event(
        r#"{"type":"note_posted","key":"k","app_id":"a","app_name":"A","title":"",
            "text":"","when_ms":1,"can_reply":false}"#,
    ) else {
        panic!("not a note");
    };
    assert_eq!(no_icon.icon, None);
    assert_eq!(
        event(r#"{"type":"note_removed","key":"k1"}"#),
        PhoneEvent::NoteRemoved { key: "k1".into() }
    );
    assert_eq!(
        event(
            r#"{"type":"sms_received","id":0,"thread_id":7,"outgoing":false,
                "text":"yo","when_ms":9}"#
        ),
        PhoneEvent::SmsReceived(SmsMessage {
            id: 0,
            thread_id: 7,
            outgoing: false,
            text: "yo".into(),
            when_ms: 9,
        })
    );
    assert_eq!(
        event(r#"{"type":"call_idle"}"#),
        PhoneEvent::Call(CallState::Idle)
    );
    assert_eq!(
        event(r#"{"type":"call_ringing","number":"+100","name":null}"#),
        PhoneEvent::Call(CallState::Ringing {
            number: "+100".into(),
            name: None,
        })
    );
    assert_eq!(
        event(r#"{"type":"call_active","since_ms":3,"muted":true}"#),
        PhoneEvent::Call(CallState::Active {
            since_ms: 3,
            muted: true,
            on_laptop_audio: false,
        })
    );
    assert_eq!(
        event(r#"{"type":"call_ended","missed":true}"#),
        PhoneEvent::Call(CallState::Ended { missed: true })
    );
    assert_eq!(
        event(r#"{"type":"dropped","count":12}"#),
        PhoneEvent::Dropped { count: 12 }
    );
}

#[test]
fn unknown_type_skipped() {
    assert!(matches!(parse(r#"{"type":"screen_on","x":1}"#), Ok(None)));
}

#[test]
fn bad_json_is_error() {
    for bad in [
        "",
        "{",
        r#"{"key":"k"}"#,
        r#"{"type":"note_removed"}"#,
        r#"{"type":"dropped","count":-1}"#,
    ] {
        assert!(
            matches!(parse(bad), Err(PlatformError::Android { .. })),
            "{bad}"
        );
    }
}

#[test]
fn lists_parse_from_glue_json() {
    let calls: Vec<CallRecord> = list(
        "CallLog.query",
        r#"[{"number":"+1","name":"Ann","when_ms":4,"duration_s":60,"kind":"declined"}]"#,
    )
    .unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].kind, CallKind::Declined);
    assert!(list::<CallRecord>("CallLog.query", "[{}]").is_err());
}
