//! Public-API tests for clipboard limits and history.

use carrova_link::{ClipHistory, ClipItem, LinkError, MAX_CLIP_PNG, MAX_CLIP_TEXT};

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
