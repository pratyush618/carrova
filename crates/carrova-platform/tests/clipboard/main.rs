//! `Platform` clipboard methods on the fake: write, read back, watch.
#![cfg(feature = "fake")]
#![allow(clippy::unwrap_used)]

use std::sync::mpsc;
use std::time::Duration;

use carrova_platform::fake::FakePlatform;
use carrova_platform::{ClipContent, ClipKind, Platform};

fn text(s: &str) -> ClipContent {
    ClipContent {
        kind: ClipKind::Text,
        bytes: s.as_bytes().to_vec(),
        sensitive: false,
    }
}

#[test]
fn clipboard_roundtrip_and_watch_fires() {
    let tmp = tempfile::tempdir().unwrap();
    let p = FakePlatform::with_home(tmp.path());
    assert_eq!(p.clipboard_read().unwrap(), None);

    let (tx, rx) = mpsc::channel();
    let watch = p
        .watch_clipboard(Box::new(move || {
            let _ = tx.send(());
        }))
        .unwrap();
    p.clipboard_write(&text("hello")).unwrap();

    rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(p.clipboard_read().unwrap(), Some(text("hello")));

    drop(watch);
    p.clipboard_write(&text("later")).unwrap();
    assert!(rx.recv_timeout(Duration::from_millis(200)).is_err());
}
