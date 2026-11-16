use carrova_link::{MESSAGE_PAGE, THREAD_PAGE};
use carrova_platform::phone::{Cursor, SmsMessage, SmsThread};
use carrova_view::phone::{LocalClock, thread_view, threads_view};

use super::{DAY, MIDNIGHT, NOON, utc_noon};

fn thread(id: i64, when_ms: i64) -> SmsThread {
    SmsThread {
        id,
        name: Some("Sam".to_owned()),
        address: "+15550142".to_owned(),
        snippet: "old".to_owned(),
        when_ms,
        unread: 0,
    }
}

fn message(id: i64, thread_id: i64, outgoing: bool, text: &str, when_ms: i64) -> SmsMessage {
    SmsMessage {
        id,
        thread_id,
        outgoing,
        text: text.to_owned(),
        when_ms,
    }
}

#[test]
fn thread_rows_count_calendar_days_back() {
    let threads = [
        thread(1, NOON - 1000),
        thread(2, MIDNIGHT - 1),
        thread(3, MIDNIGHT - 3 * DAY),
    ];
    let view = threads_view(&threads, &[], utc_noon());
    let days: Vec<u32> = view.threads.iter().map(|t| t.days_ago).collect();
    assert_eq!(days, [0, 1, 3]);
}

#[test]
fn days_follow_the_local_offset() {
    // 23:30 UTC yesterday is today at UTC+1.
    let clock = LocalClock {
        now_ms: NOON,
        utc_offset_min: 60,
    };
    let view = threads_view(&[thread(1, MIDNIGHT - DAY / 48)], &[], clock);
    assert_eq!(view.threads[0].days_ago, 0);
}

#[test]
fn future_times_count_as_today() {
    let view = threads_view(&[thread(1, NOON + 5 * DAY)], &[], utc_noon());
    assert_eq!(view.threads[0].days_ago, 0);
}

#[test]
fn newer_provisional_text_leads_its_thread() {
    let threads = [thread(1, NOON - 10_000), thread(2, NOON - 20_000)];
    let incoming = [message(0, 2, false, "new", NOON - 1000)];
    let view = threads_view(&threads, &incoming, utc_noon());
    let first = &view.threads[0];
    assert_eq!(first.id, 2);
    assert_eq!(first.snippet, "new");
    assert_eq!(first.when_ms, NOON - 1000);
    assert_eq!(first.unread, 1);
    assert_eq!(view.threads[1].snippet, "old");
}

#[test]
fn stored_or_older_texts_leave_the_thread_alone() {
    let threads = [thread(1, NOON)];
    let texts = [
        message(0, 1, false, "older", NOON - 1),
        message(7, 1, false, "stored", NOON + 1),
    ];
    let view = threads_view(&threads, &texts, utc_noon());
    assert_eq!(view.threads[0].snippet, "old");
    assert_eq!(view.threads[0].unread, 0);
}

#[test]
fn full_threads_page_offers_the_next_cursor() {
    let threads: Vec<SmsThread> = (0..THREAD_PAGE as i64)
        .map(|i| thread(100 - i, NOON - i))
        .collect();
    let last = threads.last().unwrap();
    let next = Cursor {
        when_ms: last.when_ms,
        id: last.id,
    };
    assert_eq!(threads_view(&threads, &[], utc_noon()).next, Some(next));
    assert_eq!(threads_view(&threads[..3], &[], utc_noon()).next, None);
}

#[test]
fn thread_lists_oldest_first_and_marks_provisional() {
    let page = [
        message(0, 1, false, "just now", NOON),
        message(5, 1, true, "reply", NOON - 10),
        message(4, 1, false, "hi", NOON - 20),
    ];
    let view = thread_view(&page);
    let ids: Vec<i64> = view.messages.iter().map(|m| m.id).collect();
    assert_eq!(ids, [4, 5, 0]);
    let provisional: Vec<bool> = view.messages.iter().map(|m| m.provisional).collect();
    assert_eq!(provisional, [false, false, true]);
    assert!(view.messages[1].outgoing);
    assert_eq!(view.next, None);
}

#[test]
fn full_thread_page_pages_from_its_oldest_stored_text() {
    let mut page = vec![message(0, 1, false, "provisional", NOON + 1)];
    page.extend((0..MESSAGE_PAGE as i64).map(|i| message(500 - i, 1, false, "x", NOON - i)));
    let oldest = page.last().unwrap();
    let next = Cursor {
        when_ms: oldest.when_ms,
        id: oldest.id,
    };
    assert_eq!(thread_view(&page).next, Some(next));
}
