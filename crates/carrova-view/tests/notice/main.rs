//! Which of an alert's surfaces fire: the user's preferences decide.
#![allow(clippy::unwrap_used)]

use carrova_core::{Notice, NoticeId, NoticeKind};
use carrova_platform::ScreenLock::{self, Locked, Unknown, Unlocked};
use carrova_view::ipc::{AlertChannel, AlertSettings, ErrorCode};
use carrova_view::notice::{move_news, popup_text, route};

const NOW: u64 = 1_800_000_000;
const ALL: [NoticeKind; 5] = [
    NoticeKind::SendRequest,
    NoticeKind::MoveFinished,
    NoticeKind::MoveFailed,
    NoticeKind::DeviceNearby,
    NoticeKind::UpdateReady,
];

fn everything_loud() -> AlertSettings {
    AlertSettings {
        nearby: AlertChannel::PopupAndBell,
        updates: AlertChannel::PopupAndBell,
        ..AlertSettings::default()
    }
}

#[test]
fn loud_channels_pop_up() {
    let prefs = everything_loud();
    for kind in ALL {
        assert!(route(kind, &prefs, NOW, || Unlocked).popup, "{kind:?}");
    }
}

#[test]
fn paused_alerts_never_pop_up() {
    let prefs = AlertSettings {
        paused_until: Some(NOW + 1),
        ..everything_loud()
    };
    for kind in ALL {
        assert!(!route(kind, &prefs, NOW, || Unlocked).popup, "{kind:?}");
    }
    let over = AlertSettings {
        paused_until: Some(NOW),
        ..everything_loud()
    };
    assert!(route(NoticeKind::SendRequest, &over, NOW, || Unlocked).popup);
}

#[test]
fn bell_only_never_pops_up() {
    let prefs = AlertSettings {
        requests: AlertChannel::BellOnly,
        moves: AlertChannel::BellOnly,
        ..everything_loud()
    };
    for kind in [
        NoticeKind::SendRequest,
        NoticeKind::MoveFinished,
        NoticeKind::MoveFailed,
    ] {
        let delivery = route(kind, &prefs, NOW, || Unlocked);
        assert!(!delivery.popup && !delivery.wake, "{kind:?}");
    }
}

#[test]
fn system_notifications_off_never_pop_up() {
    let prefs = AlertSettings {
        system: false,
        ..everything_loud()
    };
    assert!(!route(NoticeKind::SendRequest, &prefs, NOW, || Unlocked).popup);
}

#[test]
fn wake_only_for_send_requests() {
    let prefs = everything_loud();
    for kind in ALL {
        let wake = route(kind, &prefs, NOW, || Unlocked).wake;
        assert_eq!(wake, kind == NoticeKind::SendRequest, "{kind:?}");
    }
    let quiet = AlertSettings {
        wake_screen: false,
        ..prefs
    };
    assert!(!route(NoticeKind::SendRequest, &quiet, NOW, || Unlocked).wake);
}

#[test]
fn names_show_only_when_known_unlocked_or_allowed() {
    let hidden = AlertSettings::default();
    let shown = |prefs: &AlertSettings, lock: ScreenLock| {
        route(NoticeKind::SendRequest, prefs, NOW, || lock).show_names
    };
    assert!(shown(&hidden, Unlocked));
    assert!(!shown(&hidden, Locked));
    assert!(!shown(&hidden, Unknown));
    let allowed = AlertSettings {
        names_on_lock_screen: true,
        ..hidden
    };
    assert!(shown(&allowed, Locked));
    assert!(shown(&allowed, Unknown));
}

fn request(peer: Option<&str>) -> Notice {
    Notice {
        id: NoticeId(1),
        kind: NoticeKind::SendRequest,
        at: NOW,
        read: false,
        peer: peer.map(str::to_owned),
        summary: "wants to send you files.".into(),
    }
}

#[test]
fn pop_up_names_the_device_only_when_allowed() {
    let ana = request(Some("Ana's laptop"));
    assert_eq!(
        popup_text(&ana, true),
        "Ana's laptop wants to send you files."
    );
    assert_eq!(
        popup_text(&ana, false),
        "A known device wants to send you files."
    );
}

#[test]
fn pop_up_without_a_device_says_the_summary() {
    let mut done = request(None);
    done.summary = "The move finished.".into();
    assert_eq!(popup_text(&done, false), "The move finished.");
}

#[test]
fn a_cancelled_move_is_not_news_any_other_outcome_is() {
    assert_eq!(move_news(None).unwrap().0, NoticeKind::MoveFinished);
    assert_eq!(
        move_news(Some(ErrorCode::Network)).unwrap().0,
        NoticeKind::MoveFailed
    );
    assert_eq!(move_news(Some(ErrorCode::Cancelled)), None);
}
