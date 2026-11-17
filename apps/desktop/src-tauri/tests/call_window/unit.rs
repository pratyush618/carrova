//! Which bridge events are calls, and which calls open the window.

use carrova_core::ecosystem::CallView;

use super::{LastCall, call_view, opens_window};

/// RFC 8032 test 1's public key: a valid device id.
const DEVICE: &str = "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a";

fn payload(view: &str) -> String {
    format!(r#"{{"kind":"engine","session":"link","event":{{"event":"call_view",{view}}}}}"#)
}

#[test]
fn reads_a_call_view_event_and_nothing_else() {
    let ringing = payload(&format!(
        r#""kind":"ringing","device":"{DEVICE}","number":"+15550101","name":null,"ring_here":true"#
    ));
    assert!(matches!(
        call_view(&ringing),
        Some(CallView::Ringing {
            ring_here: true,
            ..
        })
    ));
    let link_up = format!(
        r#"{{"kind":"engine","session":"link","event":{{"event":"link_up","device":"{DEVICE}"}}}}"#
    );
    assert!(call_view(&link_up).is_none());
    assert!(call_view("not json").is_none());
}

#[test]
fn opens_for_a_call_ringing_here_or_running_only() {
    let parse = |view: &str| call_view(&payload(view)).unwrap();
    let ringing = |here: bool| {
        parse(&format!(
            r#""kind":"ringing","device":"{DEVICE}","number":"1","name":null,"ring_here":{here}"#
        ))
    };
    let active = parse(&format!(
        r#""kind":"active","device":"{DEVICE}","since_ms":0,"muted":true,"audio":"phone""#
    ));
    assert!(opens_window(&ringing(true)));
    assert!(!opens_window(&ringing(false)));
    assert!(opens_window(&active));
    assert!(!opens_window(&parse(&format!(
        r#""kind":"left","device":"{DEVICE}""#
    ))));
    assert!(!opens_window(&CallView::None));
}

#[test]
fn the_tray_names_the_caller_of_the_running_call() {
    let last = LastCall::default();
    assert!(last.running().is_none());
    last.take(
        call_view(&payload(&format!(
            r#""kind":"ringing","device":"{DEVICE}","number":"+15550101","name":"Sam","ring_here":true"#
        )))
        .unwrap(),
    );
    last.take(
        call_view(&payload(&format!(
            r#""kind":"active","device":"{DEVICE}","since_ms":0,"muted":true,"audio":"laptop""#
        )))
        .unwrap(),
    );
    let call = last.running().unwrap();
    assert!(call.muted);
    assert_eq!(call.who.as_deref(), Some("Sam"));
    last.take(CallView::None);
    assert!(last.running().is_none());
}
