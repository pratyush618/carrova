use carrova_core::ecosystem::CallView;
use carrova_crypto::identity::{DeviceId, DeviceKey};
use carrova_link::HISTORY_PAGE;
use carrova_platform::phone::{AudioSide, CallKind, CallRecord, Cursor};
use carrova_view::phone::{CallWindowView, call_log_view, call_view};

use super::{DAY, MIDNIGHT, NOON, utc_noon};

fn device() -> DeviceId {
    DeviceKey::from_bytes(&[3; 32]).public()
}

fn active(since_ms: i64, audio: AudioSide) -> CallView {
    CallView::Active {
        device: device(),
        since_ms,
        muted: false,
        audio,
    }
}

fn timer(elapsed_s: i64) -> String {
    match call_view(&active(NOON, AudioSide::Phone), NOON + elapsed_s * 1000) {
        CallWindowView::Active { timer, .. } => timer,
        other => panic!("not active: {other:?}"),
    }
}

#[test]
fn call_timer_format() {
    let cases = [
        (0, "00:00"),
        (5, "00:05"),
        (65, "01:05"),
        (59 * 60 + 59, "59:59"),
        (3600, "1:00:00"),
        (3600 + 2 * 60 + 5, "1:02:05"),
        (11 * 3600, "11:00:00"),
        // A phone clock ahead of this one.
        (-30, "00:00"),
    ];
    for (elapsed, text) in cases {
        assert_eq!(timer(elapsed), text, "{elapsed} s");
    }
}

#[test]
fn keypad_only_with_audio_here() {
    let keypad = |audio| match call_view(&active(NOON, audio), NOON) {
        CallWindowView::Active {
            keypad, audio: a, ..
        } => {
            assert_eq!(a, audio);
            keypad
        }
        other => panic!("not active: {other:?}"),
    };
    assert!(keypad(AudioSide::Laptop));
    assert!(!keypad(AudioSide::Phone));
}

#[test]
fn ringing_and_missed_show_the_caller() {
    let ringing = CallView::Ringing {
        device: device(),
        number: "+15550142".to_owned(),
        name: Some("Sam".to_owned()),
        ring_here: true,
    };
    let CallWindowView::Ringing {
        device: id,
        caller,
        ring_here,
    } = call_view(&ringing, NOON)
    else {
        panic!("not ringing");
    };
    assert_eq!(id, device().to_hex());
    assert_eq!(caller.number, "+15550142");
    assert_eq!(caller.name.as_deref(), Some("Sam"));
    assert!(ring_here);
    assert!(!format!("{caller:?}").contains("5550142"));

    let missed = CallView::Missed {
        device: device(),
        number: "+15550142".to_owned(),
        name: None,
    };
    assert!(matches!(
        call_view(&missed, NOON),
        CallWindowView::Missed { caller, .. } if caller.name.is_none()
    ));
}

#[test]
fn no_call_or_a_call_gone_closes_the_window() {
    assert_eq!(call_view(&CallView::None, NOON), CallWindowView::Closed);
    let left = CallView::Left { device: device() };
    assert_eq!(call_view(&left, NOON), CallWindowView::Closed);
}

fn record(id: i64, when_ms: i64) -> CallRecord {
    CallRecord {
        id,
        number: "+15550142".to_owned(),
        name: None,
        when_ms,
        duration_s: 240,
        kind: CallKind::Incoming,
    }
}

#[test]
fn log_groups_today_yesterday() {
    let records = [
        record(6, NOON - 1000),
        record(5, MIDNIGHT + 1),
        record(4, MIDNIGHT - 1),
        record(3, MIDNIGHT - DAY + 1),
        record(2, MIDNIGHT - 3 * DAY + 2000),
        record(1, MIDNIGHT - 3 * DAY + 1000),
    ];
    let view = call_log_view(&records, utc_noon());
    let groups: Vec<(u32, Vec<i64>)> = view
        .groups
        .iter()
        .map(|g| (g.days_ago, g.rows.iter().map(|r| r.id).collect()))
        .collect();
    assert_eq!(groups, [(0, vec![6, 5]), (1, vec![4, 3]), (3, vec![2, 1])]);
    assert_eq!(view.groups[0].rows[0].duration_s, 240);
    assert_eq!(view.next, None);
}

#[test]
fn empty_log_has_no_groups() {
    let view = call_log_view(&[], utc_noon());
    assert!(view.groups.is_empty());
    assert_eq!(view.next, None);
}

#[test]
fn full_log_page_offers_the_next_cursor() {
    let records: Vec<CallRecord> = (0..HISTORY_PAGE as i64)
        .map(|i| record(900 - i, NOON - i))
        .collect();
    let next = Cursor {
        when_ms: NOON - (HISTORY_PAGE as i64 - 1),
        id: 900 - (HISTORY_PAGE as i64 - 1),
    };
    assert_eq!(call_log_view(&records, utc_noon()).next, Some(next));
}
