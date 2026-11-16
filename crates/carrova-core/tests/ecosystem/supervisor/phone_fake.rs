//! The [`Recording`] fake's scripted phone behaves as later tests rely on.

use std::sync::{Arc, Mutex};

use carrova_platform::Platform;
use carrova_platform::fake::FakePlatform;
use carrova_platform::phone::{Note, PhoneEvent, SendOutcome};

use super::recording::Recording;

fn recording() -> Recording {
    Recording::new(FakePlatform::with_home(std::env::temp_dir()))
}

fn note(key: &str) -> Note {
    Note {
        key: key.into(),
        app_id: "app".into(),
        app_name: "App".into(),
        title: "secret title".into(),
        text: "secret text".into(),
        when_ms: 1,
        can_reply: true,
        icon: None,
        hidden: None,
    }
}

#[test]
fn events_buffer_until_watched_and_stop_on_drop() {
    let p = recording();
    let seen = Arc::new(Mutex::new(Vec::new()));
    p.script_event(PhoneEvent::Dropped { count: 1 });
    let sink = Arc::clone(&seen);
    let watch = p
        .watch_phone_events(Box::new(move |e| sink.lock().unwrap().push(e)))
        .unwrap();
    p.script_event(PhoneEvent::Dropped { count: 2 });
    drop(watch);
    p.script_event(PhoneEvent::Dropped { count: 3 });
    assert_eq!(
        *seen.lock().unwrap(),
        [
            PhoneEvent::Dropped { count: 1 },
            PhoneEvent::Dropped { count: 2 }
        ]
    );
}

#[test]
fn scripted_answers_and_content_free_actions() {
    let p = recording();
    p.phone().notes.push(note("k1"));
    assert!(p.note_reply("k1", "secret reply").unwrap());
    assert!(!p.note_reply("gone", "secret reply").unwrap());
    p.note_dismiss("k1").unwrap();
    assert!(p.active_notes().unwrap().is_empty());
    assert_eq!(p.sms_send("555", "secret").unwrap(), SendOutcome::Sent);
    p.phone().send_outcome = SendOutcome::NoSignal;
    assert_eq!(p.sms_send("555", "secret").unwrap(), SendOutcome::NoSignal);
    let actions = p.actions();
    assert_eq!(actions[0], "note_reply k1");
    assert!(
        actions
            .iter()
            .all(|a| !a.contains("secret") && !a.contains("555"))
    );
}
