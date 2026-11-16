use super::bluez::pair_target;
use super::packagekit::supported_os;
use super::wireplumber::{self, drop_in, parse_version};
use super::{AdapterInfo, status_from, valid_tones};
use crate::linux::{LinuxEnv, LinuxPlatform};
use crate::phone::CallAudioStatus as S;
use std::path::Path;
use tempfile::TempDir;

const ON: Option<AdapterInfo> = Some(AdapterInfo { powered: true });
const OFF: Option<AdapterInfo> = Some(AdapterInfo { powered: false });

#[test]
fn status_table() {
    // (adapter, hf_profile, ofono_present, paired_phone) -> status
    let rows = [
        (None, true, true, true, S::NoAdapter),
        (OFF, true, true, true, S::BluetoothOff),
        (ON, true, false, true, S::NeedsPackage),
        (ON, false, false, false, S::NeedsPackage),
        (ON, false, true, true, S::NoHandsFree),
        (ON, true, true, false, S::NeedsPairing),
        (ON, true, true, true, S::Ready),
    ];
    for (adapter, hf, ofono, paired, want) in rows {
        assert_eq!(
            status_from(adapter, hf, ofono, paired),
            want,
            "{adapter:?} hf={hf} ofono={ofono} paired={paired}"
        );
    }
}

#[test]
fn drop_in_for_04_is_lua() {
    let (path, text) = drop_in("0.4.17");
    assert_eq!(
        path,
        Path::new("wireplumber/bluetooth.lua.d/51-carrova-hfp.lua")
    );
    assert!(text.contains(r#"bluez_monitor.properties["bluez5.hfphsp-backend"] = "ofono""#));
    assert!(text.contains("hfp_hf"));
}

#[test]
fn drop_in_for_05_is_conf() {
    for version in ["0.5.0", "0.5.8", "1.0.1", "garbage"] {
        let (path, text) = drop_in(version);
        assert_eq!(
            path,
            Path::new("wireplumber/wireplumber.conf.d/51-carrova-hfp.conf"),
            "{version}"
        );
        assert!(text.contains("monitor.bluez.properties = {"));
        assert!(text.contains(r#"bluez5.hfphsp-backend = "ofono""#));
        assert!(text.contains("hfp_hf"));
    }
}

#[test]
fn version_parsed_from_linked_library() {
    let out =
        "wireplumber\nCompiled with libwireplumber 0.4.16\nLinked with libwireplumber 0.4.17\n";
    assert_eq!(parse_version(out), Some("0.4.17"));
    assert_eq!(parse_version("wireplumber\n"), None);
}

#[test]
fn write_then_remove_leaves_no_file() {
    let tmp = TempDir::new().unwrap();
    let config = tmp.path().join("config");
    let p = LinuxPlatform::new(LinuxEnv {
        home: Some(tmp.path().into()),
        xdg_config_home: Some(config.clone().into()),
        ..LinuxEnv::default()
    });
    for version in ["0.4.17", "0.5.8"] {
        let file = config.join(drop_in(version).0);
        wireplumber::write(&p, version).unwrap();
        // Idempotent: a second write replaces it in place.
        wireplumber::write(&p, version).unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), drop_in(version).1);
        let dir = file.parent().unwrap();
        assert_eq!(std::fs::read_dir(dir).unwrap().count(), 1, "no temp left");

        wireplumber::remove(&p).unwrap();
        wireplumber::remove(&p).unwrap();
        assert!(!file.exists());
    }
}

#[test]
fn install_refused_before_2404() {
    let ubuntu = |v: &str| format!("NAME=\"Ubuntu\"\nID=ubuntu\nVERSION_ID=\"{v}\"\n");
    assert!(supported_os(&ubuntu("24.04")));
    assert!(supported_os(&ubuntu("24.10")));
    assert!(supported_os(&ubuntu("26.04")));
    assert!(!supported_os(&ubuntu("22.04")));
    assert!(!supported_os(&ubuntu("23.10")));
    assert!(!supported_os("ID=debian\nVERSION_ID=\"13\"\n"));
    assert!(!supported_os(
        "ID=linuxmint\nID_LIKE=ubuntu\nVERSION_ID=\"24.04\"\n"
    ));
    assert!(!supported_os("ID=ubuntu\n"));
    assert!(!supported_os(""));
}

#[test]
fn tones_are_dtmf_digits_only() {
    assert!(valid_tones("0123456789*#"));
    assert!(!valid_tones(""));
    assert!(!valid_tones("12a"));
    assert!(!valid_tones("1 2"));
}

#[test]
fn pair_target_is_the_only_unpaired_phone() {
    let a = ("/org/bluez/hci0/dev_A", false, Some("phone"));
    let b = ("/org/bluez/hci0/dev_B", false, Some("phone"));
    let paired = ("/org/bluez/hci0/dev_C", true, Some("phone"));
    let headset = ("/org/bluez/hci0/dev_D", false, Some("audio-headset"));
    let unknown = ("/org/bluez/hci0/dev_E", false, None);
    assert_eq!(pair_target([a, paired, headset, unknown]), Some(a.0));
    assert_eq!(pair_target([a, b]), None, "two phones: ambiguous");
    assert_eq!(pair_target([paired, headset]), None);
}
