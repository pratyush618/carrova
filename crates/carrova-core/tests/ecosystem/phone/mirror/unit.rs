use std::time::Instant;

use carrova_platform::phone::SmsMessage;

use super::Mirror;

fn ok(id: i64, when_ms: i64) -> SmsMessage {
    SmsMessage {
        id,
        thread_id: 5,
        outgoing: false,
        text: "ok".into(),
        when_ms,
    }
}

fn waiting(texts: &[SmsMessage]) -> Mirror {
    let mut m = Mirror::default();
    m.provisional
        .extend(texts.iter().map(|t| (t.clone(), Instant::now())));
    m
}

#[test]
fn row_matched_once_is_not_matched_again() {
    let mut m = waiting(&[ok(0, 1_000), ok(0, 2_000)]);
    let row = ok(2, 950);

    assert_eq!(m.stored(5, std::slice::from_ref(&row)), [ok(2, 950)]);
    // A later page still holds it: the other text waits for its own row.
    assert!(m.stored(5, std::slice::from_ref(&row)).is_empty());
    assert_eq!(m.provisional.len(), 1);
}
