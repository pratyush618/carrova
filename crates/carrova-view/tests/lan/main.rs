//! LAN view-models (public API).
#![allow(clippy::unwrap_used)]

use carrova_core::PauseReason;
use carrova_core::lan::NearbyDevice;
use carrova_platform::OsKind;
use carrova_view::lan::{NearbyPeer, code_offers, pause_view};

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

#[test]
fn the_code_picker_lists_only_senders_offering_a_code() {
    let device = |name: &str, offers_code: bool| NearbyDevice {
        name: name.into(),
        os: Some(OsKind::Linux),
        known: None,
        offers_code,
        answered_ms: 10,
    };
    let found = [device("offering", true), device("present", false)];
    assert_eq!(
        code_offers(&found),
        [NearbyPeer {
            device_name: "offering".into()
        }]
    );
}
