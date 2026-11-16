//! The per-device policy store.

#![allow(clippy::unwrap_used)]

use std::path::PathBuf;

use carrova_core::CoreError;
use carrova_core::devices::{DeviceRecord, KnownDevices};
use carrova_core::ecosystem::PolicyStore;
use carrova_crypto::identity::{DeviceId, DeviceKey};
use carrova_crypto::pairing::KnownSecret;
use carrova_link::{Feature, FeatureState, PolicyDoc};
use carrova_platform::fake::FakePlatform;
use carrova_platform::{OsKind, PermissionModel, Platform};

struct Fixture {
    _tmp: tempfile::TempDir,
    state: PathBuf,
    platform: FakePlatform,
}

impl Fixture {
    fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let state = tmp.path().join("state");
        let platform = FakePlatform::with_home(tmp.path()).with_state_dir(&state);
        Self {
            _tmp: tmp,
            state,
            platform,
        }
    }

    fn file(&self, id: &DeviceId) -> PathBuf {
        self.state
            .join("devices/policy")
            .join(format!("{}.json", id.to_hex()))
    }
}

fn peer() -> DeviceId {
    DeviceKey::generate().unwrap().public()
}

#[test]
fn policy_file_mode_and_shape() {
    let f = Fixture::new();
    let store = PolicyStore::open(&f.platform).unwrap();
    let id = peer();
    store
        .set(&f.platform, &id, Feature::Clipboard, FeatureState::On)
        .unwrap();

    let path = f.file(&id);
    if f.platform.permission_model() == PermissionModel::Unix {
        let mode = f.platform.metadata(&path).unwrap().mode.unwrap();
        assert_eq!(mode & 0o777, 0o600);
    }
    let json: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(json["version"], 1);
    assert_eq!(json["features"]["clipboard"], "on");
}

#[test]
fn missing_policy_reads_computer_defaults() {
    let f = Fixture::new();
    let store = PolicyStore::open(&f.platform).unwrap();
    assert_eq!(
        store.get(&f.platform, &peer()).unwrap(),
        PolicyDoc::computer_defaults()
    );
}

#[test]
fn policy_set_rejects_ask_for_quiet() {
    let f = Fixture::new();
    let store = PolicyStore::open(&f.platform).unwrap();
    let id = peer();
    let err = store
        .set(&f.platform, &id, Feature::Clipboard, FeatureState::Ask)
        .unwrap_err();
    assert!(matches!(err, CoreError::Link(_)), "{err:?}");
    assert!(!f.file(&id).exists());
}

#[test]
fn policy_set_keeps_other_features() {
    let f = Fixture::new();
    let store = PolicyStore::open(&f.platform).unwrap();
    let id = peer();
    store
        .put(&f.platform, &id, &PolicyDoc::phone_defaults())
        .unwrap();
    let doc = store
        .set(&f.platform, &id, Feature::Ring, FeatureState::Off)
        .unwrap();
    assert_eq!(doc.get(Feature::Ring), FeatureState::Off);
    assert_eq!(doc.get(Feature::Files), FeatureState::Ask);
    assert_eq!(store.get(&f.platform, &id).unwrap(), doc);
}

/// A policy file this build cannot read: garbage, or from a newer version.
fn unreadable_policy_is_set_aside_and_replaced(bytes: &[u8]) {
    let f = Fixture::new();
    let store = PolicyStore::open(&f.platform).unwrap();
    let id = peer();
    std::fs::write(f.file(&id), bytes).unwrap();
    assert!(store.get(&f.platform, &id).is_err());

    let doc = store
        .set(&f.platform, &id, Feature::Clipboard, FeatureState::On)
        .unwrap();

    let mut want = PolicyDoc::computer_defaults();
    want.features.insert(Feature::Clipboard, FeatureState::On);
    assert_eq!(doc, want, "starts again from the defaults");
    assert_eq!(store.get(&f.platform, &id).unwrap(), doc);
    let aside: Vec<_> = std::fs::read_dir(f.file(&id).parent().unwrap())
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .filter(|name| name.contains(".corrupt-"))
        .collect();
    assert_eq!(aside.len(), 1, "the unreadable file is kept aside");
}

#[test]
fn policy_set_repairs_a_corrupt_file() {
    unreadable_policy_is_set_aside_and_replaced(b"{ not json");
}

#[test]
fn policy_set_repairs_a_newer_version_file() {
    unreadable_policy_is_set_aside_and_replaced(br#"{"version": 99, "features": {}}"#);
}

fn remember(f: &Fixture, devices: &KnownDevices, id: DeviceId) {
    let record = DeviceRecord {
        public: id,
        name: "Laptop".into(),
        os: OsKind::Linux,
        first_seen: 1,
        last_move: None,
        moves: 0,
        mac_hint: None,
    };
    devices
        .remember(&f.platform, record, &KnownSecret::from_bytes([7; 32]))
        .unwrap();
}

#[test]
fn forget_removes_policy() {
    let f = Fixture::new();
    let devices = KnownDevices::open(&f.platform).unwrap();
    let store = PolicyStore::open(&f.platform).unwrap();
    let (a, b) = (peer(), peer());
    for id in [a, b] {
        remember(&f, &devices, id);
        store
            .put(&f.platform, &id, &PolicyDoc::phone_defaults())
            .unwrap();
    }
    devices.forget(&f.platform, &a).unwrap();
    assert!(!f.file(&a).exists());
    assert!(f.file(&b).exists());

    devices.forget_all(&f.platform).unwrap();
    assert!(!f.file(&b).exists());
}

#[test]
fn reset_identity_removes_policies() {
    let f = Fixture::new();
    let devices = KnownDevices::open(&f.platform).unwrap();
    let store = PolicyStore::open(&f.platform).unwrap();
    let id = peer();
    store
        .put(&f.platform, &id, &PolicyDoc::phone_defaults())
        .unwrap();
    devices.reset_identity(&f.platform).unwrap();
    assert!(!f.file(&id).exists());
}

#[test]
fn existing_known_computer_defaults_all_off() {
    let f = Fixture::new();
    let devices = KnownDevices::open(&f.platform).unwrap();
    let id = peer();
    remember(&f, &devices, id);
    let store = PolicyStore::open(&f.platform).unwrap();
    let doc = store.get(&f.platform, &id).unwrap();
    assert!(
        Feature::PHASE2
            .iter()
            .all(|&x| doc.get(x) == FeatureState::Off)
    );
}
