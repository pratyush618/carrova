//! The queue of links the app was opened with, and which links may join it.

use std::path::Path;

use tauri::Url;

use super::{Opened, admitted};

const SHARED: &str = "/data/user/0/org.byteveda.Carrova/files/cache/shared";

fn admits(link: &str) -> bool {
    admitted(&Url::parse(link).unwrap(), Some(Path::new(SHARED)))
}

#[test]
fn queued_links_are_taken_once_in_arrival_order() {
    let opened = Opened::default();
    let links =
        ["carrova://send-clipboard", "https://example.org/a"].map(|l| Url::parse(l).unwrap());

    assert_eq!(opened.push_admitted(&links[..1], None), 0);
    assert_eq!(opened.push_admitted(&links[1..], None), 0);

    assert_eq!(
        opened.take(),
        ["carrova://send-clipboard", "https://example.org/a"]
    );
    assert!(opened.take().is_empty());
}

#[test]
fn pushing_drops_and_counts_refused_links() {
    let opened = Opened::default();
    let links = ["file:///etc/passwd", "https://example.org/"].map(|l| Url::parse(l).unwrap());

    assert_eq!(opened.push_admitted(&links, Some(Path::new(SHARED))), 1);

    assert_eq!(opened.take(), ["https://example.org/"]);
}

#[test]
fn the_clipboard_request_and_shared_text_are_admitted() {
    assert!(admits("carrova://send-clipboard"));
    assert!(admits("https://example.org/page?q=1"));
    assert!(admits("http://example.org/"));
    assert!(admits("data:text/plain,hello%20there"));
}

#[test]
fn share_copies_are_admitted() {
    assert!(admits(&format!("file://{SHARED}/0/holiday%20photo.jpg")));
}

#[test]
fn private_app_files_are_refused() {
    let files = "/data/user/0/org.byteveda.Carrova/files";
    assert!(!admits(&format!("file://{files}/state/devices/key")));
    assert!(!admits(&format!("file://{SHARED}/../../state/devices/key")));
    assert!(!admits(&format!(
        "file://{SHARED}/0/..%2F..%2F..%2Fstate%2Fkey"
    )));
    assert!(!admits(&format!("file://{SHARED}")));
    assert!(!admits(&format!("file://{SHARED}-evil/key")));
    assert!(!admits("file:///etc/passwd"));
}

#[test]
fn file_links_are_refused_without_a_share_dir() {
    let link = Url::parse(&format!("file://{SHARED}/0/a.txt")).unwrap();
    assert!(!admitted(&link, None));
}

#[test]
fn other_links_are_refused() {
    assert!(!admits(
        "content://org.byteveda.Carrova.fileprovider/data/state/key"
    ));
    assert!(!admits("content://com.example.provider/photo/1"));
    assert!(!admits("carrova://send-files"));
    assert!(!admits("data:text/html,<b>hi</b>"));
    assert!(!admits("javascript:alert(1)"));
    assert!(!admits("file://otherhost/share/x"));
}

/// What queuing `link` leaves for the UI.
fn queued(link: &str) -> Vec<String> {
    let opened = Opened::default();
    opened.push_admitted(&[Url::parse(link).unwrap()], None);
    opened.take()
}

#[test]
fn shared_text_is_queued_in_one_plain_form() {
    let plain = "data:text/plain,a%20b%23c%E2%82%AC";
    for link in [
        "data:text/plain,a%20b%23c%E2%82%AC",
        "data:text/plain;charset=UTF-8,a%20b%23c%E2%82%AC",
        "data:TEXT/PLAIN;charset=utf-8;base64,YSBiI2Pigqw=",
        "data:text/plain;base64,YSBiI2Pigqw%3D",
        "data:text/plain,a b#c€",
    ] {
        assert_eq!(queued(link), [plain], "{link}");
    }
}

#[test]
fn shared_text_in_another_charset_or_not_utf8_is_refused() {
    assert!(!admits("data:text/plain;charset=iso-8859-1,caf%E9"));
    assert!(!admits("data:text/plain,caf%E9"));
    assert!(!admits("data:text/plain;base64,not base64!"));
    assert!(!admits("data:text/plain;name=x,hi"));
    assert!(queued("data:text/plain;base64,%FF").is_empty());
}
