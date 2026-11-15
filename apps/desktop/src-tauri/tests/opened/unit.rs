//! The queue of links the app was opened with.

use super::Opened;

#[test]
fn queued_links_are_taken_once_oldest_first() {
    let opened = Opened::default();
    opened.push(["carrova://send-clipboard".to_owned()]);
    opened.push(["file:///a".to_owned(), "file:///b".to_owned()]);

    assert_eq!(
        opened.take(),
        ["carrova://send-clipboard", "file:///a", "file:///b"]
    );
    assert!(opened.take().is_empty());
}
