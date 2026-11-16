//! Public-API tests for the notification lane.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[path = "../support/mod.rs"]
mod support;

use std::thread;

use carrova_link::{
    ActionResult, Incoming, LaneKind, LinkError, MAX_ICON, MAX_NOTE_TEXT, MAX_SNAPSHOT,
    NotifyAction, NotifyMsg, accept_lane, fit_note, recv_notify, send_action, send_notify,
};
use carrova_platform::phone::Note;
use support::{Open, T, linked};

fn note(text: &str) -> Note {
    Note {
        key: "k".into(),
        app_id: "app".into(),
        app_name: "App".into(),
        title: "t".into(),
        text: text.into(),
        when_ms: 1,
        can_reply: true,
        icon: None,
        hidden: None,
    }
}

/// Send `msg`; return the sender's result and what the receiver got.
fn push(msg: &NotifyMsg) -> (Result<(), LinkError>, Result<Incoming, LinkError>) {
    let (a, b) = linked();
    thread::scope(|s| {
        let server = s.spawn(|| {
            let (kind, stream) = accept_lane(&b, &Open, T).unwrap();
            assert_eq!(kind, LaneKind::Notify);
            recv_notify(stream)
        });
        let sent = send_notify(&a, msg);
        (sent, server.join().unwrap())
    })
}

fn got(msg: &NotifyMsg) -> NotifyMsg {
    match push(msg).1.unwrap() {
        Incoming::Msg(m) => m,
        Incoming::Action(..) => panic!("expected a message"),
    }
}

#[test]
fn posted_roundtrip() {
    let msg = NotifyMsg::Posted(note("hi"));
    assert_eq!(got(&msg), msg);
    let gone = NotifyMsg::Removed { key: "k".into() };
    assert_eq!(got(&gone), gone);
}

#[test]
fn snapshot_capped_at_50() {
    let ok = NotifyMsg::Snapshot(vec![note("x"); MAX_SNAPSHOT]);
    assert_eq!(got(&ok), ok);
    let (sent, recv) = push(&NotifyMsg::Snapshot(vec![note("x"); MAX_SNAPSHOT + 1]));
    assert!(matches!(recv, Err(LinkError::TooBig { .. })));
    assert!(sent.is_err());
}

#[test]
fn oversize_text_refused_on_receive() {
    let big = "a".repeat(MAX_NOTE_TEXT + 1);
    let (_, recv) = push(&NotifyMsg::Posted(note(&big)));
    assert!(matches!(recv, Err(LinkError::TooBig { .. })));
}

#[test]
fn fit_cuts_on_char_boundary() {
    // 4095 ASCII bytes then a 3-byte char straddling the 4096 edge.
    let text = format!("{}€", "a".repeat(MAX_NOTE_TEXT - 1));
    let fitted = fit_note(note(&text));
    assert_eq!(fitted.text, "a".repeat(MAX_NOTE_TEXT - 1));
}

#[test]
fn oversize_icon_dropped() {
    let mut n = note("x");
    n.icon = Some(vec![0; MAX_ICON + 1]);
    assert_eq!(fit_note(n).icon, None);
    let mut n = note("x");
    n.icon = Some(vec![0; MAX_ICON]);
    assert!(fit_note(n).icon.is_some());
}

#[test]
fn reply_result_roundtrip() {
    let (a, b) = linked();
    let action = NotifyAction::Reply {
        key: "k".into(),
        text: "ok".into(),
    };
    let result = thread::scope(|s| {
        let server = s.spawn(|| {
            let (_, stream) = accept_lane(&b, &Open, T).unwrap();
            match recv_notify(stream).unwrap() {
                Incoming::Action(got, responder) => {
                    assert_eq!(got, action);
                    responder.reply(&ActionResult::Gone).unwrap();
                }
                Incoming::Msg(_) => panic!("expected an action"),
            }
        });
        let result = send_action(&a, &action).unwrap();
        server.join().unwrap();
        result
    });
    assert_eq!(result, ActionResult::Gone);
}

#[test]
fn oversize_reply_refused_on_receive() {
    let (a, b) = linked();
    thread::scope(|s| {
        let server = s.spawn(|| {
            let (_, stream) = accept_lane(&b, &Open, T).unwrap();
            recv_notify(stream)
        });
        let _ = send_action(
            &a,
            &NotifyAction::Reply {
                key: "k".into(),
                text: "a".repeat(4097),
            },
        );
        assert!(matches!(
            server.join().unwrap(),
            Err(LinkError::TooBig { .. })
        ));
    });
}

#[test]
fn debug_hides_content() {
    let shown = format!("{:?}", NotifyMsg::Posted(note("secret-text")));
    assert!(!shown.contains("secret-text"));
}
