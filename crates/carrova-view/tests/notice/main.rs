//! Which of an alert's surfaces fire: the user's preferences decide.
#![allow(clippy::unwrap_used)]

use carrova_core::NoticeKind;
use carrova_platform::ScreenLock::{self, Locked, Unknown, Unlocked};
use carrova_view::ipc::{AlertChannel, AlertSettings};
use carrova_view::notice::route;

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
