//! Public API of `carrova_core::devices`: the known-device store.

#![allow(clippy::unwrap_used)]

use std::path::{Path, PathBuf};

use carrova_core::devices::{DeviceRecord, KnownDevices};
use carrova_core::{CoreError, KeyFileProblem};
use carrova_crypto::identity::{DeviceKey, DevicePublic, Fingerprint};
use carrova_crypto::pairing::{KNOWN_SECRET_LEN, KnownSecret};
use carrova_platform::fake::FakePlatform;
use carrova_platform::{OsKind, PermissionModel, Platform};

struct Fixture {
    _tmp: tempfile::TempDir,
    state: PathBuf,
    platform: FakePlatform,
    devices: KnownDevices,
}

impl Fixture {
    fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let state = tmp.path().join("state");
        let platform = FakePlatform::with_home(tmp.path()).with_state_dir(&state);
        let devices = KnownDevices::open(&platform).unwrap();
        Self {
            _tmp: tmp,
            state,
            platform,
            devices,
        }
    }

    fn key_file(&self, public: &DevicePublic) -> PathBuf {
        self.state
            .join("devices")
            .join(format!("{}.key", public.to_hex()))
    }

    fn known_json(&self) -> PathBuf {
        self.state.join("devices/known.json")
    }

    fn remember(&self, record: &DeviceRecord, secret: u8) {
        self.devices
            .remember(&self.platform, record.clone(), &known(secret))
            .unwrap();
    }

    fn secret(&self, public: &DevicePublic) -> Option<[u8; KNOWN_SECRET_LEN]> {
        self.devices
            .secret_for(&self.platform, public)
            .unwrap()
            .map(|s| *s.expose_bytes())
    }
}

fn public(seed: u8) -> DevicePublic {
    DeviceKey::from_bytes(&[seed; 32]).public()
}

fn known(byte: u8) -> KnownSecret {
    KnownSecret::from_bytes([byte; KNOWN_SECRET_LEN])
}

fn record(seed: u8, name: &str) -> DeviceRecord {
    DeviceRecord {
        public: public(seed),
        name: name.into(),
        os: OsKind::Linux,
        first_seen: 100,
        last_move: None,
        moves: 0,
        mac_hint: Some([0xde, 0xad, 0xbe, 0xef, 0x00, 0x01]),
    }
}

/// Asserts `path` has exactly `expected` permission bits, where the
/// platform has them (ACLs decide access elsewhere).
fn assert_mode(platform: &dyn Platform, path: &Path, expected: u32) {
    if unix(platform) {
        assert_eq!(
            platform.metadata(path).unwrap().mode.unwrap() & 0o777,
            expected
        );
    }
}

/// Whether the platform keeps permission bits (not ACLs).
fn unix(platform: &dyn Platform) -> bool {
    platform.permission_model() == PermissionModel::Unix
}

#[test]
fn remember_then_list() {
    let f = Fixture::new();
    f.remember(&record(1, "laptop"), 7);
    f.remember(&record(2, "desk"), 8);
    assert_eq!(
        f.devices.list(&f.platform).unwrap(),
        vec![record(1, "laptop"), record(2, "desk")]
    );
    assert_eq!(f.secret(&public(1)), Some([7; KNOWN_SECRET_LEN]));
    assert_eq!(f.secret(&public(3)), None);
}

#[test]
fn remembering_again_replaces_the_secret_and_keeps_name_and_history() {
    let f = Fixture::new();
    f.remember(&record(1, "laptop"), 7);
    f.devices
        .rename(&f.platform, &public(1), "work laptop".into())
        .unwrap();
    let again = DeviceRecord {
        first_seen: 500,
        os: OsKind::MacOs,
        mac_hint: None,
        ..record(1, "hostname")
    };
    f.remember(&again, 9);
    let listed = f.devices.list(&f.platform).unwrap();
    assert_eq!(
        listed,
        vec![DeviceRecord {
            name: "work laptop".into(),
            first_seen: 100,
            ..again
        }]
    );
    assert_eq!(f.secret(&public(1)), Some([9; KNOWN_SECRET_LEN]));
    assert!(!f.key_file(&public(1)).with_extension("key.new").exists());
}

#[test]
fn two_stores_on_one_dir_lose_no_remember() {
    let f = Fixture::new();
    let other = FakePlatform::with_home(f.state.parent().unwrap()).with_state_dir(&f.state);
    let (first, second) = (0..20u8).partition::<Vec<_>, _>(|seed| seed % 2 == 0);
    std::thread::scope(|scope| {
        for (platform, seeds) in [(&f.platform, first), (&other, second)] {
            scope.spawn(move || {
                let devices = KnownDevices::open(platform).unwrap();
                for seed in seeds {
                    devices
                        .remember(platform, record(seed + 1, "peer"), &known(seed))
                        .unwrap();
                }
            });
        }
    });
    assert_eq!(f.devices.list(&f.platform).unwrap().len(), 20);
}

#[test]
fn replaced_secrets_are_zeroed_on_disk() {
    let f = Fixture::new();
    f.remember(&record(1, "laptop"), 7);
    f.devices.device_key(&f.platform).unwrap();
    // A second name for each old file sees what stays on disk.
    let old_secret = f.state.join("old-secret");
    let old_seed = f.state.join("old-seed");
    std::fs::hard_link(f.key_file(&public(1)), &old_secret).unwrap();
    std::fs::hard_link(f.state.join("device.key"), &old_seed).unwrap();
    f.remember(&record(1, "laptop"), 9);
    f.devices.reset_identity(&f.platform).unwrap();
    assert_eq!(std::fs::read(old_secret).unwrap(), [0; KNOWN_SECRET_LEN]);
    assert_eq!(std::fs::read(old_seed).unwrap(), [0; 32]);
}

#[test]
fn forget_removes_key_file() {
    let f = Fixture::new();
    f.remember(&record(1, "laptop"), 7);
    f.remember(&record(2, "desk"), 8);
    assert!(f.devices.forget(&f.platform, &public(1)).unwrap());
    assert!(!f.key_file(&public(1)).exists());
    assert_eq!(
        f.devices.list(&f.platform).unwrap(),
        vec![record(2, "desk")]
    );
    assert_eq!(f.secret(&public(1)), None);
    assert!(!f.devices.forget(&f.platform, &public(1)).unwrap());
}

#[test]
fn forget_all_removes_every_record_and_key() {
    let f = Fixture::new();
    f.remember(&record(1, "laptop"), 7);
    f.remember(&record(2, "desk"), 8);
    f.devices.forget_all(&f.platform).unwrap();
    assert_eq!(f.devices.list(&f.platform).unwrap(), vec![]);
    assert!(!f.key_file(&public(1)).exists() && !f.key_file(&public(2)).exists());
}

#[test]
fn secret_without_a_record_is_not_used() {
    let f = Fixture::new();
    f.remember(&record(1, "laptop"), 7);
    std::fs::copy(f.key_file(&public(1)), f.key_file(&public(2))).unwrap();
    assert_eq!(f.secret(&public(2)), None);
}

#[test]
fn files_are_0600() {
    let f = Fixture::new();
    f.remember(&record(1, "laptop"), 7);
    f.devices.device_key(&f.platform).unwrap();
    assert_mode(&f.platform, &f.state.join("device.key"), 0o600);
    assert_mode(&f.platform, &f.known_json(), 0o600);
    assert_mode(&f.platform, &f.key_file(&public(1)), 0o600);
    assert_mode(&f.platform, &f.state.join("devices"), 0o700);
}

#[test]
fn key_file_others_can_read_is_refused() {
    let f = Fixture::new();
    if !unix(&f.platform) {
        return; // ACLs, not mode bits, keep the state dir private there.
    }
    f.remember(&record(1, "laptop"), 7);
    f.platform.set_mode(&f.key_file(&public(1)), 0o644).unwrap();
    let err = f.devices.secret_for(&f.platform, &public(1)).unwrap_err();
    assert!(
        matches!(
            err,
            CoreError::DeviceKey {
                problem: KeyFileProblem::Mode { .. },
                ..
            }
        ),
        "{err:?}"
    );
    assert!(!err.to_string().contains(&public(1).to_hex()), "{err}");
}

#[test]
fn rename_keeps_identity() {
    let f = Fixture::new();
    f.remember(&record(1, "laptop"), 7);
    f.devices
        .rename(&f.platform, &public(1), "studio".into())
        .unwrap();
    let listed = f.devices.list(&f.platform).unwrap();
    assert_eq!(listed[0].public, public(1));
    assert_eq!(listed[0].name, "studio");
    assert_eq!(f.secret(&public(1)), Some([7; KNOWN_SECRET_LEN]));
}

#[test]
fn rename_of_unknown_device_fails() {
    let f = Fixture::new();
    let err = f
        .devices
        .rename(&f.platform, &public(1), "studio".into())
        .unwrap_err();
    assert!(matches!(err, CoreError::UnknownDevice), "{err:?}");
}

#[test]
fn corrupt_store_is_reported_not_overwritten() {
    let f = Fixture::new();
    f.remember(&record(1, "laptop"), 7);
    std::fs::write(f.known_json(), b"{ not json").unwrap();
    for err in [
        f.devices.list(&f.platform).unwrap_err(),
        f.devices
            .remember(&f.platform, record(2, "desk"), &known(8))
            .unwrap_err(),
        f.devices.forget(&f.platform, &public(1)).unwrap_err(),
        f.devices
            .rename(&f.platform, &public(1), "x".into())
            .unwrap_err(),
    ] {
        assert!(matches!(err, CoreError::CorruptFile { .. }), "{err:?}");
    }
    assert_eq!(std::fs::read(f.known_json()).unwrap(), b"{ not json");
    assert!(f.key_file(&public(1)).exists());
    assert!(!f.key_file(&public(2)).exists());
}

#[test]
fn newer_store_version_is_reported_and_unknown_fields_are_ignored() {
    let f = Fixture::new();
    f.remember(&record(1, "laptop"), 7);
    let text = std::fs::read_to_string(f.known_json()).unwrap();
    let mut doc: serde_json::Value = serde_json::from_str(&text).unwrap();
    doc["future"] = serde_json::json!(true);
    doc["devices"][0]["future"] = serde_json::json!(1);
    std::fs::write(f.known_json(), doc.to_string()).unwrap();
    assert_eq!(
        f.devices.list(&f.platform).unwrap(),
        vec![record(1, "laptop")]
    );
    doc["version"] = serde_json::json!(99);
    std::fs::write(f.known_json(), doc.to_string()).unwrap();
    let err = f.devices.list(&f.platform).unwrap_err();
    assert!(matches!(err, CoreError::CorruptFile { .. }), "{err:?}");
}

#[test]
fn device_key_is_made_once_and_reloaded() {
    let f = Fixture::new();
    let first = f.devices.device_key(&f.platform).unwrap().public();
    let reopened = KnownDevices::open(&f.platform).unwrap();
    assert_eq!(reopened.device_key(&f.platform).unwrap().public(), first);
}

#[test]
fn partial_device_key_from_a_crash_is_discarded() {
    let f = Fixture::new();
    std::fs::write(f.state.join("device.key.new"), b"short").unwrap();
    let key = f.devices.device_key(&f.platform).unwrap().public();
    assert!(!f.state.join("device.key.new").exists());
    assert_eq!(f.devices.device_key(&f.platform).unwrap().public(), key);
}

#[test]
fn corrupt_device_key_is_reported_not_regenerated() {
    let f = Fixture::new();
    f.devices.device_key(&f.platform).unwrap();
    let path = f.state.join("device.key");
    std::fs::write(&path, b"short").unwrap();
    let err = f.devices.device_key(&f.platform).unwrap_err();
    assert!(
        matches!(
            err,
            CoreError::DeviceKey {
                problem: KeyFileProblem::WrongSize,
                ..
            }
        ),
        "{err:?}"
    );
    assert_eq!(std::fs::read(&path).unwrap(), b"short");
}

#[test]
fn reset_identity_changes_key_and_forgets_all() {
    let f = Fixture::new();
    let old = f.devices.device_key(&f.platform).unwrap().public();
    f.remember(&record(1, "laptop"), 7);
    let new = f.devices.reset_identity(&f.platform).unwrap().public();
    assert_ne!(new, old);
    assert_eq!(f.devices.device_key(&f.platform).unwrap().public(), new);
    assert_eq!(f.devices.list(&f.platform).unwrap(), vec![]);
    assert!(!f.key_file(&public(1)).exists());
    assert_mode(&f.platform, &f.state.join("device.key"), 0o600);
}

#[test]
fn resolve_accepts_a_fingerprint_or_the_full_key() {
    let f = Fixture::new();
    f.remember(&record(1, "laptop"), 7);
    f.remember(&record(2, "desk"), 8);
    let fp = Fingerprint::of(&public(2)).to_string();
    for query in [
        fp.clone(),
        fp.replace('\u{b7}', "").to_lowercase(),
        public(2).to_hex(),
    ] {
        assert_eq!(
            f.devices.resolve(&f.platform, &query).unwrap(),
            record(2, "desk"),
            "{query}"
        );
    }
}

#[test]
fn resolve_fails_for_no_match_or_several() {
    let f = Fixture::new();
    f.remember(&record(1, "laptop"), 7);
    for query in [
        Fingerprint::of(&public(2)).to_string(),
        public(2).to_hex(),
        "laptop".into(),
    ] {
        let err = f.devices.resolve(&f.platform, &query).unwrap_err();
        assert!(matches!(err, CoreError::UnknownDevice), "{query}: {err:?}");
    }
    // A hand-edited store listing one key twice: never pick one silently.
    let text = std::fs::read_to_string(f.known_json()).unwrap();
    let mut doc: serde_json::Value = serde_json::from_str(&text).unwrap();
    let first = doc["devices"][0].clone();
    doc["devices"].as_array_mut().unwrap().push(first);
    std::fs::write(f.known_json(), doc.to_string()).unwrap();
    let err = f
        .devices
        .resolve(&f.platform, &Fingerprint::of(&public(1)).to_string())
        .unwrap_err();
    assert!(
        matches!(err, CoreError::AmbiguousDevice { matches: 2 }),
        "{err:?}"
    );
}

#[test]
fn record_debug_has_no_mac_or_full_key() {
    let shown = format!("{:?}", record(1, "laptop"));
    assert!(!shown.contains(&public(1).to_hex()[..16]), "{shown}");
    assert!(
        !shown.contains("mac_hint") && !shown.contains("222, 173"),
        "{shown}"
    );
    assert!(
        shown.contains(&Fingerprint::of(&public(1)).to_string()),
        "{shown}"
    );
}
