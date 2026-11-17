use carrova_link::SendOutcome;

use super::{SENT_KEPT, SentTexts};

#[test]
fn repeat_is_answered_from_the_first_send() {
    let mut sent = SentTexts::default();
    assert_eq!(sent.begin(7), None);
    assert_eq!(sent.begin(7), Some(SendOutcome::Unknown));
    sent.finish(7, SendOutcome::Sent);
    assert_eq!(sent.begin(7), Some(SendOutcome::Sent));
}

#[test]
fn oldest_text_is_forgotten_past_the_bound() {
    let mut sent = SentTexts::default();
    for id in 0..=u64::try_from(SENT_KEPT).unwrap() {
        assert_eq!(sent.begin(id), None);
    }
    assert_eq!(sent.begin(0), None);
}
