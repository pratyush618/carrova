//! Public-API tests for clipboard limits and history.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[path = "../support/mod.rs"]
mod support;

use std::thread;

use carrova_link::{
    ClipHistory, ClipItem, LaneKind, LinkError, MAX_CLIP_PNG, MAX_CLIP_TEXT, accept_lane,
    open_lane, recv_clip, send_clip,
};
use serde::Serialize;
use support::{Open, T, linked};

/// The clipboard header as any peer may put it on the wire.
#[derive(Serialize)]
struct RawHeader {
    kind: &'static str,
    len: u64,
}

/// Send `item` and receive it on the other side.
fn roundtrip(item: &ClipItem) -> ClipItem {
    let (a, b) = linked();
    thread::scope(|s| {
        let server = s.spawn(|| {
            let (kind, stream) = accept_lane(&b, &Open, T).unwrap();
            assert_eq!(kind, LaneKind::Clipboard);
            recv_clip(stream).unwrap()
        });
        send_clip(&a, item).unwrap();
        server.join().unwrap()
    })
}

/// Open a clipboard lane, send `header` and `payload` raw, return the receiver's result.
fn raw(header: &RawHeader, payload: &[u8]) -> Result<ClipItem, LinkError> {
    let (a, b) = linked();
    thread::scope(|s| {
        let server = s.spawn(|| recv_clip(accept_lane(&b, &Open, T).unwrap().1));
        let mut stream = open_lane(&a, LaneKind::Clipboard).unwrap();
        stream.send(header).unwrap();
        stream.write_all(payload, T).unwrap();
        server.join().unwrap()
    })
}

fn text(n: usize) -> ClipItem {
    ClipItem::Text("a".repeat(n))
}

#[test]
fn oversize_text_refused_by_sender() {
    assert!(matches!(
        text(MAX_CLIP_TEXT + 1).check(),
        Err(LinkError::TooBig {
            max: MAX_CLIP_TEXT,
            ..
        })
    ));
}

#[test]
fn oversize_png_refused_by_sender() {
    let png = ClipItem::Png(vec![0; MAX_CLIP_PNG + 1]);
    assert!(matches!(
        png.check(),
        Err(LinkError::TooBig {
            max: MAX_CLIP_PNG,
            ..
        })
    ));
}

#[test]
fn items_at_exactly_the_cap_pass() {
    assert!(text(MAX_CLIP_TEXT).check().is_ok());
    assert!(ClipItem::Png(vec![0; MAX_CLIP_PNG]).check().is_ok());
}

#[test]
fn history_keeps_twenty_newest() {
    let mut h = ClipHistory::default();
    for i in 0..25 {
        h.push(ClipItem::Text(i.to_string()));
    }
    let items: Vec<_> = h.items().collect();
    assert_eq!(items.len(), 20);
    assert_eq!(items[0], &ClipItem::Text("24".into()));
    assert_eq!(items[19], &ClipItem::Text("5".into()));
    h.clear();
    assert_eq!(h.items().count(), 0);
}

#[test]
fn debug_output_never_shows_clip_contents() {
    let text = format!("{:?}", ClipItem::Text("hunter2".into()));
    let png = format!("{:?}", ClipItem::Png(vec![0xAB; 3]));
    assert!(!text.contains("hunter2") && text.contains("Text") && text.contains('7'));
    assert!(!png.contains("171") && png.contains("Png") && png.contains('3'));
}

#[test]
fn text_roundtrip() {
    let item = ClipItem::Text("héllo\nwörld".into());
    assert_eq!(roundtrip(&item), item);
}

#[test]
fn png_roundtrip() {
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    png.extend((0..=255).cycle().take(3 << 20));
    let item = ClipItem::Png(png);
    assert_eq!(roundtrip(&item), item);
}

#[test]
fn oversize_rejected_by_receiver_even_if_sender_lies() {
    // Declares more than the cap and sends nothing: the receiver must refuse
    // from the header alone instead of waiting for (or buffering) the payload.
    for (kind, max) in [("Text", MAX_CLIP_TEXT), ("Png", MAX_CLIP_PNG)] {
        let header = RawHeader {
            kind,
            len: max as u64 + 1,
        };
        assert!(
            matches!(raw(&header, b""), Err(LinkError::TooBig { max: m, .. }) if m == max),
            "{kind}"
        );
    }
}

#[test]
fn receiver_refuses_malformed_payloads() {
    let bad_text = RawHeader {
        kind: "Text",
        len: 2,
    };
    assert!(matches!(
        raw(&bad_text, &[0xff, 0xfe]),
        Err(LinkError::Protocol(_))
    ));
    let bad_png = RawHeader {
        kind: "Png",
        len: 8,
    };
    assert!(matches!(
        raw(&bad_png, b"GIF89a.."),
        Err(LinkError::Protocol(_))
    ));
}
