//! The undo of a pairing the peer did not complete.

#![allow(clippy::unwrap_used)]

use carrova_crypto::identity::DeviceKey;
use carrova_crypto::pairing::KnownSecret;
use carrova_link::{Feature, FeatureState};
use carrova_platform::fake::FakePlatform;
use carrova_transport::{PairedPeer, PeerIdentity};

use super::*;

fn peer(public: DevicePublic, secret_byte: u8) -> PairedPeer {
    PairedPeer {
        identity: PeerIdentity {
            public,
            secret: KnownSecret::from_bytes([secret_byte; 32]),
        },
        name: "Peer".to_owned(),
        os: OsKind::Linux,
    }
}

fn platform() -> (tempfile::TempDir, FakePlatform) {
    let tmp = tempfile::tempdir().unwrap();
    let p = FakePlatform::with_home(tmp.path()).with_state_dir(tmp.path().join("state"));
    (tmp, p)
}

#[test]
fn undo_forgets_a_new_device_with_its_secret_and_policy() {
    let (_tmp, p) = platform();
    let public = DeviceKey::generate().unwrap().public();

    store(&p, &peer(public, 1)).unwrap().undo(&p);

    let known = KnownDevices::open(&p).unwrap();
    assert!(known.list(&p).unwrap().is_empty());
    assert!(known.secret_for(&p, &public).unwrap().is_none());
    let policies = PolicyStore::open(&p).unwrap();
    assert!(policies.get(&p, &public).unwrap() == PolicyDoc::computer_defaults());
}

#[test]
fn undo_restores_a_re_paired_device() {
    let (_tmp, p) = platform();
    let public = DeviceKey::generate().unwrap().public();
    let first = store(&p, &peer(public, 1)).unwrap().record;
    PolicyStore::open(&p)
        .unwrap()
        .set(&p, &public, Feature::Clipboard, FeatureState::On)
        .unwrap();

    store(&p, &peer(public, 2)).unwrap().undo(&p);

    let known = KnownDevices::open(&p).unwrap();
    assert_eq!(known.list(&p).unwrap(), vec![first]);
    let secret = known.secret_for(&p, &public).unwrap().unwrap();
    assert_eq!(secret.expose_bytes(), &[1; 32]);
    let doc = PolicyStore::open(&p).unwrap().get(&p, &public).unwrap();
    assert_eq!(doc.features[&Feature::Clipboard], FeatureState::On);
}
