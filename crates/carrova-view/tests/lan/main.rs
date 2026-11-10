//! LAN view-models (public API).
#![allow(clippy::unwrap_used)]

use carrova_core::PauseReason;
use carrova_view::lan::pause_view;

#[test]
fn a_pause_the_user_asked_for_has_nothing_to_explain() {
    assert!(pause_view(&PauseReason::ByUser, None).is_none());
}

#[test]
fn a_low_disk_space_pause_shows_the_shortfall() {
    let reason = PauseReason::LowDiskSpace {
        fs: "/state".into(),
        available: 10,
        needed: 25,
        disk: None,
    };
    assert_eq!(pause_view(&reason, None).unwrap().shortfall, 15);
}
