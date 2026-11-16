use super::bluez::{Seen, linked_phone, on_adapter, pair_target};
use super::ofono::{card_for, tone_modem};
use super::packagekit::{Outcome, outcome, supported_os};
use super::pipewire::hands_free_up;
use super::wireplumber::{self, drop_in, parse_version};
use super::{AdapterInfo, bus_err, status_from, valid_tones};
use crate::PlatformError;
use crate::linux::{LinuxEnv, LinuxPlatform};
use crate::phone::CallAudioStatus as S;
use std::path::Path;
use tempfile::TempDir;

const ON: Option<AdapterInfo> = Some(AdapterInfo { powered: true });
const OFF: Option<AdapterInfo> = Some(AdapterInfo { powered: false });

#[test]
fn status_table() {
    // (adapter, hf_profile, ofono_present, configured, paired_phone) -> status
    let rows = [
        (None, true, true, true, true, S::NoAdapter),
        (OFF, true, true, true, true, S::BluetoothOff),
        (ON, true, false, true, true, S::NeedsPackage),
        (ON, false, false, false, false, S::NeedsPackage),
        // oFono there but the drop-in missing: still the install screen,
        // never blamed on the hardware.
        (ON, false, true, false, true, S::NeedsPackage),
        (ON, true, true, false, true, S::NeedsPackage),
        (ON, false, true, true, true, S::NoHandsFree),
        (ON, true, true, true, false, S::NeedsPairing),
        (ON, true, true, true, true, S::Ready),
    ];
    for (adapter, hf, ofono, configured, paired, want) in rows {
        assert_eq!(
            status_from(adapter, hf, ofono, configured, paired),
            want,
            "{adapter:?} hf={hf} ofono={ofono} configured={configured} paired={paired}"
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
    }
}

#[test]
fn drop_in_keeps_default_bluetooth_roles() {
    // An explicit role list is a filter: it would switch off LE Audio.
    for version in ["0.4.17", "0.5.8"] {
        assert!(!drop_in(version).1.contains("bluez5.roles"), "{version}");
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
        assert!(wireplumber::configured(&p, version).unwrap());
        let dir = file.parent().unwrap();
        assert_eq!(std::fs::read_dir(dir).unwrap().count(), 1, "no temp left");

        wireplumber::remove(&p).unwrap();
        wireplumber::remove(&p).unwrap();
        assert!(!file.exists());
        assert!(!wireplumber::configured(&p, version).unwrap());
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
    assert_eq!(pair_target([a, paired, headset, unknown]), Seen::One(a.0));
    assert_eq!(pair_target([a, b]), Seen::Several, "two phones: ambiguous");
    assert_eq!(pair_target([paired, headset]), Seen::None);
}

#[test]
fn devices_belong_to_their_own_adapter_only() {
    assert!(on_adapter("/org/bluez/hci0", "/org/bluez/hci0/dev_AA_BB"));
    assert!(!on_adapter("/org/bluez/hci0", "/org/bluez/hci1/dev_AA_BB"));
    // A prefix of the adapter's name is another adapter.
    assert!(!on_adapter("/org/bluez/hci1", "/org/bluez/hci10/dev_AA_BB"));
    assert!(!on_adapter("/org/bluez/hci0", "/org/bluez/hci0"));
}

#[test]
fn linked_phone_prefers_the_connected_gateway() {
    // (path, paired with the audio gateway role, connected, address)
    let idle = ("/dev_A", true, false, "AA");
    let live = ("/dev_B", true, true, "BB");
    let unpaired = ("/dev_C", false, true, "CC");
    assert_eq!(linked_phone([idle, live, unpaired]), Some(("/dev_B", "BB")));
    assert_eq!(linked_phone([unpaired, idle]), Some(("/dev_A", "AA")));
    assert_eq!(linked_phone([unpaired]), None);
}

#[test]
fn tones_go_to_the_modem_in_a_call() {
    let pick = |a, b| tone_modem([("/hfp/a", a), ("/hfp/b", b)]);
    assert_eq!(pick(false, true), Some("/hfp/b"));
    assert_eq!(pick(true, true), Some("/hfp/a"));
    // No call anywhere: the first, as before.
    assert_eq!(pick(false, false), Some("/hfp/a"));
    assert_eq!(tone_modem([]), None);
}

#[test]
fn card_is_the_hands_free_card_of_that_phone() {
    // (path, type, remote address). oFono's hands-free plugin creates
    // "handsfree" cards; "gateway" ones are its audio-gateway role.
    let other = ("/card_1", Some("handsfree"), Some("11:22"));
    let mine = ("/card_2", Some("handsfree"), Some("aa:bb"));
    let ag = ("/card_3", Some("gateway"), Some("AA:BB"));
    assert_eq!(card_for([other, ag, mine], "AA:BB"), Some("/card_2"));
    assert_eq!(card_for([other, ag], "AA:BB"), None);
}

#[test]
fn install_outcome_tells_cancel_from_failure() {
    const SUCCESS: u32 = 1;
    const FAILED: u32 = 2;
    const CANCELLED: u32 = 3;
    // PkErrorEnum in PackageKit 1.2 (what Ubuntu ships).
    const NOT_AUTHORIZED: u32 = 48;
    const NO_SPACE_ON_DEVICE: u32 = 46;
    const TRANSACTION_CANCELLED: u32 = 17;
    const NO_NETWORK: u32 = 2;
    assert_eq!(outcome(SUCCESS, None), Outcome::Done);
    assert_eq!(outcome(CANCELLED, None), Outcome::Cancelled);
    assert_eq!(outcome(FAILED, Some(NOT_AUTHORIZED)), Outcome::Cancelled);
    assert_eq!(
        outcome(FAILED, Some(TRANSACTION_CANCELLED)),
        Outcome::Cancelled
    );
    assert_eq!(outcome(FAILED, Some(NO_NETWORK)), Outcome::Failed);
    assert_eq!(outcome(FAILED, Some(NO_SPACE_ON_DEVICE)), Outcome::Failed);
    assert_eq!(outcome(FAILED, None), Outcome::Failed);
}

#[test]
fn bus_errors_keep_only_the_error_name() {
    let secret = "/org/bluez/hci0/dev_AA_BB_CC_DD_EE_FF";
    let fdo = zbus::Error::FDO(Box::new(zbus::fdo::Error::AccessDenied(secret.into())));
    let PlatformError::Io { source, .. } = bus_err("org.bluez", fdo) else {
        panic!("not an I/O error");
    };
    let text = source.to_string();
    assert!(!text.contains("dev_AA"), "{text}");
    assert!(text.contains("AccessDenied"), "{text}");
}

/// A `pw-dump` graph: one phone with its hands-free node up, a headphone
/// on A2DP only.
const DUMP: &str = include_str!("pw-dump.json");

#[test]
fn hands_free_node_puts_audio_here() {
    assert!(hands_free_up(DUMP, "AA:BB:CC:DD:EE:01"));
    assert!(hands_free_up(DUMP, "aa:bb:cc:dd:ee:01"));
}

#[test]
fn no_hands_free_node_leaves_audio_on_the_phone() {
    // Only an A2DP node, a device with no node, and garbage.
    assert!(!hands_free_up(DUMP, "AA:BB:CC:DD:EE:02"));
    assert!(!hands_free_up(DUMP, "AA:BB:CC:DD:EE:03"));
    assert!(!hands_free_up("not json", "AA:BB:CC:DD:EE:01"));
    assert!(!hands_free_up("{}", "AA:BB:CC:DD:EE:01"));
}
