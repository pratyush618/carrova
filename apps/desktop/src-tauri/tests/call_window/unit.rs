//! Which bridge events are calls, and the running call the tray offers.

use carrova_core::ecosystem::CallView;

use super::{LastCall, call_view};

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
fn the_tray_follows_the_running_call() {
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
    assert!(last.running().unwrap().muted);
    last.take(CallView::None);
    assert!(last.running().is_none());
}
