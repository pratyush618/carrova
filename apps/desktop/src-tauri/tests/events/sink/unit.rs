#![allow(clippy::unwrap_used)]

use carrova_core::State;

use super::*;
use crate::testing::{Recorder, Refreshes};

fn id(s: &str) -> SessionId {
    SessionId(s.into())
}

fn warning(message: &str) -> Event {
    Event::Warning {
        message: message.into(),
    }
}

#[test]
fn events_carry_their_session() {
    let recorder = Arc::new(Recorder::default());
    let sink = SessionSink::bound(recorder.clone(), Arc::new(Refreshes::default()), &id("s1"));
    sink.emit(warning("hello"));
    let events = recorder.events();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["kind"], "engine");
    assert_eq!(events[0]["session"], "s1");
    assert_eq!(events[0]["event"]["event"], "warning");
}

#[test]
fn events_before_the_session_is_known_are_delivered_in_order_on_bind() {
    let recorder = Arc::new(Recorder::default());
    let sink = SessionSink::unbound(recorder.clone(), Arc::new(Refreshes::default()));
    sink.emit(warning("first"));
    sink.emit(warning("second"));
    assert!(recorder.events().is_empty());
    sink.bind(&id("s1"));
    let messages: Vec<_> = recorder
        .events()
        .iter()
        .map(|e| e["event"]["message"].clone())
        .collect();
    assert_eq!(messages, ["first", "second"]);
}

#[test]
fn the_first_state_change_names_the_session() {
    let recorder = Arc::new(Recorder::default());
    let sink = SessionSink::unbound(recorder.clone(), Arc::new(Refreshes::default()));
    sink.emit(warning("early"));
    sink.emit(Event::StateChanged {
        session: id("s2"),
        from: State::Applying,
        to: State::RolledBack,
    });
    let events = recorder.events();
    assert_eq!(events.len(), 2);
    assert!(events.iter().all(|e| e["session"] == "s2"));
    assert_eq!(events[1]["event"]["event"], "state_changed");
}

#[test]
fn a_remembered_device_refreshes_the_presence_advert() {
    let presence = Arc::new(Refreshes::default());
    let sink = SessionSink::bound(Arc::new(Recorder::default()), presence.clone(), &id("s1"));
    sink.emit(warning("not this"));
    assert_eq!(presence.count(), 0);
    sink.emit(Event::DeviceRemembered {
        name: "Desk".into(),
        fingerprint: "7F3A·19C2".into(),
    });
    assert_eq!(presence.count(), 1);
}
