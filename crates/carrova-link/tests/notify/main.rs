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
use carrova_transport::MAX_FRAME_LEN;
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

/// Run the receiving side against `send`; the message it got, acknowledged.
fn exchange<S>(send: S) -> Result<NotifyMsg, LinkError>
where
    S: FnOnce(&carrova_transport::Connection) + Send,
{
    let (a, b) = linked();
    thread::scope(|s| {
        let server = s.spawn(|| {
            let (kind, stream) = accept_lane(&b, &Open, T).unwrap();
            assert_eq!(kind, LaneKind::Notify);
            match recv_notify(stream)? {
                Incoming::Msg(m, ack) => {
                    ack.ack()?;
                    Ok(m)
                }
                Incoming::Action(..) => panic!("expected a message"),
            }
        });
        send(&a);
        server.join().unwrap()
    })
}

/// Send `msg` through the real sender.
fn push(msg: &NotifyMsg) -> Result<NotifyMsg, LinkError> {
    exchange(|a| {
        let _ = send_notify(a, msg);
    })
}

/// Put `msg` on the wire bypassing the sender's checks.
fn push_raw(msg: NotifyMsg) -> Result<NotifyMsg, LinkError> {
    exchange(|a| {
        let mut stream = open_lane(a, LaneKind::Notify).unwrap();
        stream.send(&Frame::Msg(msg)).unwrap();
        let _ = stream.recv::<()>(T);
    })
}

fn got(msg: &NotifyMsg) -> NotifyMsg {
    push(msg).unwrap()
}

fn got_raw(msg: NotifyMsg) -> NotifyMsg {
    push_raw(msg).unwrap()
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
            let Incoming::Msg(_, ack) = recv_notify(stream).unwrap() else {
                panic!("expected a message")
            };
            ack.ack().unwrap();
        });
        send_notify(&a, &NotifyMsg::Removed { key: "k".into() }).unwrap();
    });
}

#[test]
fn message_unacknowledged_until_the_receiver_acks() {
    // The sender's next message waits for this one to be applied.
    let (a, b) = linked();
    thread::scope(|s| {
        s.spawn(|| {
            let (_, stream) = accept_lane(&b, &Open, T).unwrap();
            drop(recv_notify(stream).unwrap());
        });
        let sent = send_notify(&a, &NotifyMsg::Removed { key: "k".into() });
        assert!(sent.is_err(), "{sent:?}");
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
            n.title = "t".repeat(MAX_NOTE_TITLE);
            n.when_ms = i64::try_from(i).unwrap();
            n.icon = Some(vec![0xAB; MAX_ICON]);
            n
        })
        .collect();
    let fitted = NotifyMsg::Snapshot(fit_snapshot(notes.clone()));
    let mut wire = Vec::new();
    ciborium::into_writer(&Frame::Msg(fitted), &mut wire).unwrap();
    assert!(wire.len() <= MAX_FRAME_LEN, "{} bytes", wire.len());

    let NotifyMsg::Snapshot(received) = got(&NotifyMsg::Snapshot(notes.clone())) else {
        panic!("expected a snapshot");
    };
    assert_eq!(received.len(), MAX_SNAPSHOT);
    // Newest first; the newest keep their icons, the oldest lose them.
    assert_eq!(received[0].when_ms, 49);
    assert!(received[0].icon.is_some());
    assert!(received.last().unwrap().icon.is_none());
    for (recv, sent) in received.iter().zip(notes.iter().rev()) {
        assert_eq!(sent.text, recv.text);
    }
    let total: usize = received
        .iter()
        .filter_map(|n| n.icon.as_ref())
        .map(Vec::len)
        .sum();
    assert!(total <= 512 * 1024);
}

#[test]
fn fit_snapshot_keeps_the_newest_fifty() {
    let notes: Vec<Note> = (0..70)
        .map(|i| {
            let mut n = note("x");
            n.when_ms = i;
            n
        })
        .collect();
    let fitted = fit_snapshot(notes);
    assert_eq!(fitted.len(), MAX_SNAPSHOT);
    assert_eq!(fitted.first().unwrap().when_ms, 69);
    assert_eq!(fitted.last().unwrap().when_ms, 20);
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

fn long_key() -> String {
    "k".repeat(257)
}

/// Every frame variant that carries a notification key, with that key over-long.
fn long_key_messages() -> [NotifyMsg; 1] {
    [NotifyMsg::Removed { key: long_key() }]
}

fn long_key_actions() -> [NotifyAction; 3] {
    [
        NotifyAction::Dismiss { key: long_key() },
        NotifyAction::Open { key: long_key() },
        NotifyAction::Reply {
            key: long_key(),
            text: "hi".into(),
        },
    ]
}

#[test]
fn sender_refuses_long_keys() {
    let (a, _b) = linked();
    for msg in long_key_messages() {
        assert!(matches!(
            send_notify(&a, &msg),
            Err(LinkError::TooBig { .. })
        ));
    }
    for action in long_key_actions() {
        assert!(matches!(
            send_action(&a, &action),
            Err(LinkError::TooBig { .. })
        ));
    }
}

#[test]
fn receiver_refuses_long_keys() {
    #[derive(Serialize)]
    enum RawFrame {
        Msg(NotifyMsg),
        Action(NotifyAction),
    }
    let raw = |frame: RawFrame| {
        let (a, b) = linked();
        thread::scope(|s| {
            let server = s.spawn(|| {
                let (_, stream) = accept_lane(&b, &Open, T).unwrap();
                recv_notify(stream)
            });
            let mut stream = open_lane(&a, LaneKind::Notify).unwrap();
            stream.send(&frame).unwrap();
            server.join().unwrap()
        })
    };
    let frames = long_key_messages()
        .map(RawFrame::Msg)
        .into_iter()
        .chain(long_key_actions().map(RawFrame::Action));
    for frame in frames {
        assert!(matches!(raw(frame), Err(LinkError::TooBig { .. })));
    }
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
                Incoming::Msg(..) => panic!("expected an action"),
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
