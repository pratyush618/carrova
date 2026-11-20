//! What a move request shows before Review: counts only, and on a locked
//! screen not even those.

use carrova_platform::ScreenLock::{Locked, Unknown, Unlocked};
use carrova_view::request::{MoveRequestView, RequestSummary, request_popup, summary_text};

const NOW: u64 = 1_800_000_000;

fn request() -> MoveRequestView {
    MoveRequestView {
        id: 1,
        device: "d2".into(),
        name: "Desk PC".into(),
        summary: RequestSummary {
            apps: 12,
            bytes: 340_000_000,
            has_sensitive: true,
        },
        expires_at: NOW + 102,
    }
}

#[test]
fn summary_counts_apps_size_and_keys() {
    assert_eq!(
        summary_text(&request().summary),
        "12 apps · 340.0 MB · includes keys"
    );
}

#[test]
fn summary_without_keys_or_with_one_app_says_so() {
    let summary = RequestSummary {
        apps: 1,
        bytes: 999,
        has_sensitive: false,
    };
    assert_eq!(summary_text(&summary), "1 app · 999 B");
}

#[test]
fn unlocked_popup_names_the_computer_with_counts_and_time_left() {
    let popup = request_popup(&request(), NOW, Unlocked, false);
    assert_eq!(popup.title, "Desk PC wants to send you its setup");
    assert_eq!(
        popup.body,
        "12 apps · 340.0 MB · includes keys. Nothing changes until you review it. 1:42 left."
    );
}

#[test]
fn locked_notice_hides_counts_and_name_by_default() {
    for lock in [Locked, Unknown] {
        let popup = request_popup(&request(), NOW, lock, false);
        assert_eq!(popup.title, "A trusted computer: 1 request", "{lock:?}");
        assert!(!popup.body.contains("12 apps"), "{lock:?}: {}", popup.body);
        assert!(!popup.body.contains("Desk PC"), "{lock:?}: {}", popup.body);
        assert!(
            popup.body.ends_with("It expires in 1:42."),
            "{}",
            popup.body
        );
    }
}

#[test]
fn locked_notice_names_the_computer_only_when_allowed() {
    let popup = request_popup(&request(), NOW, Locked, true);
    assert_eq!(popup.title, "Desk PC: 1 request");
    assert!(!popup.body.contains("12 apps"), "{}", popup.body);
}

#[test]
fn expired_request_shows_no_time_left() {
    let popup = request_popup(&request(), NOW + 500, Locked, false);
    assert!(
        popup.body.ends_with("It expires in 0:00."),
        "{}",
        popup.body
    );
}
