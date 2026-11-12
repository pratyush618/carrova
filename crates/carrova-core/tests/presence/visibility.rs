//! The visibility the user chose, kept in the state dir so the CLI and the
//! desktop app share one value.

use carrova_core::presence::{EVERYONE_FOR, Visibility, load_visibility, save_visibility};
use carrova_platform::Platform;

use super::NOW;
use crate::lan_support::Side;

fn visibility_file(side: &Side) -> std::path::PathBuf {
    side.platform.state_dir().unwrap().join("presence.json")
}

#[test]
fn old_or_missing_file_reads_known() {
    let side = Side::new();
    assert_eq!(
        load_visibility(&*side.platform, NOW).unwrap(),
        Visibility::Known
    );
    std::fs::create_dir_all(side.platform.state_dir().unwrap()).unwrap();
    std::fs::write(visibility_file(&side), br#"{"kind":"friends"}"#).unwrap();
    assert_eq!(
        load_visibility(&*side.platform, NOW).unwrap(),
        Visibility::Known
    );
}

#[test]
fn a_saved_visibility_reads_back() {
    let side = Side::new();
    for visibility in [
        Visibility::NoOne,
        Visibility::Known,
        Visibility::Everyone { until: NOW + 60 },
    ] {
        save_visibility(&*side.platform, visibility).unwrap();
        assert_eq!(load_visibility(&*side.platform, NOW).unwrap(), visibility);
    }
}

#[test]
fn past_deadline_reads_known() {
    let side = Side::new();
    save_visibility(&*side.platform, Visibility::Everyone { until: NOW }).unwrap();
    assert_eq!(
        load_visibility(&*side.platform, NOW).unwrap(),
        Visibility::Known
    );
}

#[test]
fn everyone_lasts_ten_minutes_from_now() {
    assert_eq!(EVERYONE_FOR.as_secs(), 600);
    assert_eq!(
        Visibility::everyone_from(NOW),
        Visibility::Everyone { until: NOW + 600 }
    );
}
