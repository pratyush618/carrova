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
        sent.finish(id, SendOutcome::Sent);
    }
    assert_eq!(sent.begin(0), None);
}

#[test]
fn a_text_still_sending_is_never_forgotten() {
    let kept = u64::try_from(SENT_KEPT).unwrap();
    let mut sent = SentTexts::default();
    for id in 0..kept {
        assert_eq!(sent.begin(id), None);
    }
    // Full of texts still sending: a new one is refused, not sent.
    assert_eq!(sent.begin(kept), Some(SendOutcome::Failed));
    assert_eq!(sent.begin(0), Some(SendOutcome::Unknown));

    // A finished one makes room; the oldest, still sending, stays.
    sent.finish(5, SendOutcome::Sent);
    assert_eq!(sent.begin(kept), None);
    assert_eq!(sent.begin(0), Some(SendOutcome::Unknown));
    // 5 was forgotten and the rest still send: a new text again.
    assert_eq!(sent.begin(5), Some(SendOutcome::Failed));
}
