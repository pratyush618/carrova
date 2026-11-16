//! Public-API tests for the notification lane.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[path = "../support/mod.rs"]
mod support;

use std::thread;

use carrova_link::{
    ActionResult, Incoming, LaneKind, LinkError, MAX_ICON, MAX_NOTE_TEXT, MAX_NOTE_TITLE,
    MAX_REPLY, MAX_SNAPSHOT, NotifyAction, NotifyMsg, accept_lane, fit_note, fit_snapshot,
    open_lane, recv_notify, send_action, send_notify,
};
use carrova_platform::phone::Note;
use serde::Serialize;
use support::{Open, T, linked};

/// Mirror of the private first frame, to put over-limit data on the wire.
#[derive(Serialize)]
enum Frame {
    Msg(NotifyMsg),
}

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

/// Run the receiving side against `send`.
fn exchange<S>(send: S) -> Result<Incoming, LinkError>
where
    S: FnOnce(&carrova_transport::Connection) + Send,
{
    let (a, b) = linked();
    thread::scope(|s| {
        let server = s.spawn(|| {
            let (kind, stream) = accept_lane(&b, &Open, T).unwrap();
            assert_eq!(kind, LaneKind::Notify);
            recv_notify(stream)
        });
        send(&a);
        server.join().unwrap()
    })
}

/// Send `msg` through the real sender.
fn push(msg: &NotifyMsg) -> Result<Incoming, LinkError> {
    exchange(|a| {
        let _ = send_notify(a, msg);
    })
}

/// Put `msg` on the wire bypassing the sender's checks.
fn push_raw(msg: NotifyMsg) -> Result<Incoming, LinkError> {
    exchange(|a| {
        let mut stream = open_lane(a, LaneKind::Notify).unwrap();
        stream.send(&Frame::Msg(msg)).unwrap();
        let _ = stream.recv::<()>(T);
    })
}

fn got(msg: &NotifyMsg) -> NotifyMsg {
    match push(msg).unwrap() {
        Incoming::Msg(m) => m,
        Incoming::Action(..) => panic!("expected a message"),
    }
}

fn got_raw(msg: NotifyMsg) -> NotifyMsg {
    match push_raw(msg).unwrap() {
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
fn message_is_acked_with_unit() {
    let (a, b) = linked();
    thread::scope(|s| {
        s.spawn(|| {
            let (_, stream) = accept_lane(&b, &Open, T).unwrap();
            recv_notify(stream).unwrap();
        });
        send_notify(&a, &NotifyMsg::Removed { key: "k".into() }).unwrap();
    });
}

#[test]
fn snapshot_capped_at_50() {
    let ok = NotifyMsg::Snapshot(vec![note("x"); MAX_SNAPSHOT]);
    assert_eq!(got(&ok), ok);
    // The sender fits to 50; a peer that doesn't is refused on receive.
    let over = vec![note("x"); MAX_SNAPSHOT + 1];
    assert_eq!(got(&NotifyMsg::Snapshot(over.clone())), ok);
    assert!(matches!(
        push_raw(NotifyMsg::Snapshot(over)),
        Err(LinkError::TooBig { .. })
    ));
}

#[test]
fn full_snapshot_fits_the_frame_cap() {
    let notes: Vec<Note> = (0..MAX_SNAPSHOT)
        .map(|i| {
            let mut n = note(&"a".repeat(MAX_NOTE_TEXT));
            n.when_ms = i64::try_from(i).unwrap();
            n.icon = Some(vec![7; MAX_ICON]);
            n
        })
        .collect();
    let NotifyMsg::Snapshot(received) = got(&NotifyMsg::Snapshot(notes.clone())) else {
        panic!("expected a snapshot");
    };
    assert_eq!(received.len(), MAX_SNAPSHOT);
    for (sent, recv) in notes.iter().zip(&received) {
        assert_eq!(sent.text, recv.text);
    }
    // Newest keep their icons; the oldest lose them.
    assert!(received.last().unwrap().icon.is_some());
    assert!(received[0].icon.is_none());
    let total: usize = received
        .iter()
        .filter_map(|n| n.icon.as_ref())
        .map(Vec::len)
        .sum();
    assert!(total <= 512 * 1024);
}

#[test]
fn fit_snapshot_drops_oldest_icons_first() {
    let mut notes = Vec::new();
    // Newest first in the vec, to prove it sorts by `when_ms`, not position.
    for when in (0..10).rev() {
        let mut n = note("x");
        n.when_ms = when;
        n.icon = Some(vec![0; MAX_ICON]);
        notes.push(n);
    }
    let fitted = fit_snapshot(notes);
    let kept: Vec<i64> = fitted
        .iter()
        .filter(|n| n.icon.is_some())
        .map(|n| n.when_ms)
        .collect();
    assert_eq!(kept.len(), 8); // 8 x 64 KiB = 512 KiB
    assert!(kept.iter().all(|w| *w >= 2));
}

#[test]
fn oversize_text_refused_on_receive() {
    let big = "a".repeat(MAX_NOTE_TEXT + 1);
    assert!(matches!(
        push_raw(NotifyMsg::Posted(note(&big))),
        Err(LinkError::TooBig { .. })
    ));
}

#[test]
fn oversize_title_refused_on_receive() {
    let mut n = note("x");
    n.title = "t".repeat(MAX_NOTE_TITLE + 1);
    assert!(matches!(
        push_raw(NotifyMsg::Posted(n)),
        Err(LinkError::TooBig { .. })
    ));
}

#[test]
fn oversize_identity_fields_refused_on_receive() {
    for field in 0..3 {
        let mut n = note("x");
        let long = "k".repeat(257);
        match field {
            0 => n.key = long,
            1 => n.app_id = long,
            _ => n.app_name = long,
        }
        assert!(matches!(
            push_raw(NotifyMsg::Posted(n)),
            Err(LinkError::TooBig { .. })
        ));
    }
}

#[test]
fn oversize_icon_dropped_on_receive() {
    let mut n = note("x");
    n.icon = Some(vec![0; MAX_ICON + 1]);
    let NotifyMsg::Posted(back) = got_raw(NotifyMsg::Posted(n.clone())) else {
        panic!("expected posted");
    };
    n.icon = None;
    assert_eq!(back, n);
}

#[test]
fn sender_refuses_oversize_before_sending() {
    let (a, _b) = linked();
    let big = "a".repeat(MAX_NOTE_TEXT + 1);
    assert!(matches!(
        send_notify(&a, &NotifyMsg::Posted(note(&big))),
        Err(LinkError::TooBig { .. })
    ));
    let action = NotifyAction::Reply {
        key: "k".into(),
        text: "a".repeat(MAX_REPLY + 1),
    };
    assert!(matches!(
        send_action(&a, &action),
        Err(LinkError::TooBig { .. })
    ));
}

#[test]
fn fit_cuts_on_char_boundary() {
    // 4095 ASCII bytes then a 3-byte char straddling the 4096 edge.
    let text = format!("{}€", "a".repeat(MAX_NOTE_TEXT - 1));
    let fitted = fit_note(note(&text));
    assert_eq!(fitted.text, "a".repeat(MAX_NOTE_TEXT - 1));
}

#[test]
fn fit_cuts_title() {
    let mut n = note("x");
    n.title = "é".repeat(MAX_NOTE_TITLE); // 512 bytes
    assert_eq!(fit_note(n).title, "é".repeat(MAX_NOTE_TITLE / 2));
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
    #[derive(Serialize)]
    enum RawFrame {
        #[allow(dead_code)]
        Msg(NotifyMsg),
        Action(NotifyAction),
    }
    let (a, b) = linked();
    thread::scope(|s| {
        let server = s.spawn(|| {
            let (_, stream) = accept_lane(&b, &Open, T).unwrap();
            recv_notify(stream)
        });
        let mut stream = open_lane(&a, LaneKind::Notify).unwrap();
        stream
            .send(&RawFrame::Action(NotifyAction::Reply {
                key: "k".into(),
                text: "a".repeat(MAX_REPLY + 1),
            }))
            .unwrap();
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
