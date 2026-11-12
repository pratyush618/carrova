//! Devices are remembered after a code-paired move: two engines in one
//! process over loopback.

use carrova_core::devices::{DEVICE_KEY_FILE, DeviceRecord, KnownDevices};
use carrova_core::{CoreError, Event, Importer, PlanOptions, State};
use carrova_crypto::identity::{DevicePublic, Fingerprint};
use carrova_platform::OsKind;

use super::lan_support::{Side, run, run_answering};

fn options() -> PlanOptions {
    PlanOptions {
        confirmed_sensitive: vec!["app/keys".into()],
        ..PlanOptions::default()
    }
}

/// A full code-paired move from `sender` to `receiver`.
pub(super) fn moved(sender: &Side, receiver: &Side) {
    let mut exp = sender.selected_exporter();
    let running = run(exp.prepare_send().unwrap());
    let mut imp =
        Importer::connect(receiver.engine.clone(), running.first.as_ref().unwrap()).unwrap();
    imp.plan(&options()).unwrap();
    imp.approve().unwrap();
    imp.apply().unwrap();
    assert_eq!(imp.state(), State::Done);
    running.thread.join().unwrap().unwrap();
}

fn state_dir(side: &Side) -> std::path::PathBuf {
    side.tmp.path().join("state/carrova")
}

fn store(side: &Side) -> KnownDevices {
    KnownDevices::open(&*side.platform).unwrap()
}

pub(super) fn known(side: &Side) -> Vec<DeviceRecord> {
    store(side).list(&*side.platform).unwrap()
}

fn identity(side: &Side) -> DevicePublic {
    store(side).device_key(&*side.platform).unwrap().public()
}

fn secret_with(side: &Side, peer: &DevicePublic) -> [u8; 32] {
    *store(side)
        .secret_for(&*side.platform, peer)
        .unwrap()
        .unwrap()
        .expose_bytes()
}

fn remembered(side: &Side) -> Vec<(String, String)> {
    side.sink
        .events()
        .into_iter()
        .filter_map(|e| match e {
            Event::DeviceRemembered { name, fingerprint } => Some((name, fingerprint)),
            _ => None,
        })
        .collect()
}

#[test]
fn paired_move_remembers_both_sides() {
    let sender = Side::seeded(Side::named(Some("Studio desktop")));
    let receiver = Side::named(Some("Travel laptop"));
    moved(&sender, &receiver);

    let (sender_id, receiver_id) = (identity(&sender), identity(&receiver));
    let [by_sender] = known(&sender).try_into().unwrap();
    let [by_receiver] = known(&receiver).try_into().unwrap();
    assert_eq!(by_sender.public, receiver_id);
    assert_eq!(by_receiver.public, sender_id);
    assert_eq!(by_sender.name, "Travel laptop");
    assert_eq!(by_receiver.name, "Studio desktop");
    for record in [&by_sender, &by_receiver] {
        assert_eq!(record.os, OsKind::Linux);
        assert_eq!(record.moves, 1);
        assert!(record.last_move.is_some_and(|t| t >= record.first_seen));
        assert_eq!(record.mac_hint, None);
    }
    assert_eq!(
        secret_with(&sender, &receiver_id),
        secret_with(&receiver, &sender_id)
    );
    assert_eq!(
        remembered(&sender),
        [(
            "Travel laptop".to_owned(),
            Fingerprint::of(&receiver_id).to_string()
        )]
    );
    assert_eq!(
        remembered(&receiver),
        [(
            "Studio desktop".to_owned(),
            Fingerprint::of(&sender_id).to_string()
        )]
    );
}

#[test]
fn pairing_again_replaces_the_secret_and_counts_the_move() {
    let sender = Side::sender();
    let receiver = Side::new();
    moved(&sender, &receiver);
    let receiver_id = identity(&receiver);
    let first = secret_with(&sender, &receiver_id);
    moved(&sender, &receiver);

    let [record] = known(&sender).try_into().unwrap();
    assert_eq!(record.moves, 2);
    assert_ne!(secret_with(&sender, &receiver_id), first);
    assert_eq!(
        secret_with(&sender, &receiver_id),
        secret_with(&receiver, &identity(&sender))
    );
}

#[test]
fn remember_off_stores_nothing() {
    for off_on_sender in [false, true] {
        let mut sender = Side::sender();
        let mut receiver = Side::new();
        let off = if off_on_sender {
            &mut sender
        } else {
            &mut receiver
        };
        off.engine = off.engine.clone().with_remember_devices(false);
        moved(&sender, &receiver);

        for side in [&sender, &receiver] {
            assert!(known(side).is_empty());
            assert!(remembered(side).is_empty());
        }
        let off = if off_on_sender { &sender } else { &receiver };
        assert!(
            !state_dir(off).join(DEVICE_KEY_FILE).exists(),
            "no identity is made while remembering is off"
        );
    }
}

#[test]
fn unreadable_identity_still_moves_and_remembers_nothing() {
    for corrupt_on_sender in [false, true] {
        let sender = Side::sender();
        let receiver = Side::new();
        let corrupt = if corrupt_on_sender {
            &sender
        } else {
            &receiver
        };
        std::fs::create_dir_all(state_dir(corrupt)).unwrap();
        std::fs::write(state_dir(corrupt).join(DEVICE_KEY_FILE), b"garbage").unwrap();
        moved(&sender, &receiver);

        for side in [&sender, &receiver] {
            assert!(known(side).is_empty());
            assert!(remembered(side).is_empty());
        }
        assert!(
            corrupt
                .sink
                .events()
                .iter()
                .any(|e| matches!(e, Event::Warning { .. })),
            "the user hears this computer will not be remembered"
        );
    }
}

#[test]
fn peer_proving_this_devices_own_identity_fails_the_session() {
    let sender = Side::sender();
    let receiver = Side::new();
    // A cloned state dir: both sides hold the same device key.
    identity(&sender);
    std::fs::create_dir_all(state_dir(&receiver)).unwrap();
    std::fs::copy(
        state_dir(&sender).join(DEVICE_KEY_FILE),
        state_dir(&receiver).join(DEVICE_KEY_FILE),
    )
    .unwrap();

    let mut exp = sender.selected_exporter();
    let running = run_answering(exp.prepare_send().unwrap(), false);
    let err =
        Importer::connect(receiver.engine.clone(), running.first.as_ref().unwrap()).unwrap_err();
    assert!(matches!(err, CoreError::Transport(_)), "{err}");
    let (result, _, _) = running.finish();
    assert!(
        matches!(result, Err(CoreError::PairingStopped { failures: 1 })),
        "{result:?}"
    );
    assert!(known(&sender).is_empty());
    assert!(known(&receiver).is_empty());
}
