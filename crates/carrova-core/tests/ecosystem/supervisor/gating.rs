//! Two-sided policy gating for files, handoff and ring: the sender refuses
//! what the peer turned off ([`CoreError::FeatureOff`]), and the receiver
//! refuses the lane itself when a sender acts on a stale policy (dialled
//! outside any supervisor here, so no sender check runs).

use carrova_core::CoreError;
use carrova_core::ecosystem::{HandoffItem, PeerState, Supervisor};
use carrova_link::Feature;
use carrova_link::{
    FeatureState, FileOffer, Handoff, LinkError, PolicyDoc, RefuseReason, RingMsg, offer_file,
    send_handoff, send_ring,
};
use carrova_transport::Connection;

use super::{asks, link_down, link_up, linked, wait_remote};
use crate::supervisor::device::{Device, fast, pair};

/// Low and high linked under phone defaults, then high turns `feature`
/// off for low and low has heard it.
fn sender_sees_off(feature: Feature) -> (Device, Device, Supervisor, Supervisor) {
    let (low, high, sl, sh) = linked(&PolicyDoc::phone_defaults());
    sh.set_feature(&low.id, feature, FeatureState::Off).unwrap();
    wait_remote(&sl, &high.id, feature, FeatureState::Off);
    (low, high, sl, sh)
}

fn assert_feature_off<T: std::fmt::Debug>(sent: &Result<T, CoreError>, feature: Feature) {
    assert!(
        matches!(sent, Err(CoreError::FeatureOff(f)) if *f == feature),
        "{sent:?}"
    );
}

#[test]
fn files_off_on_the_peer_refuses_the_send() {
    let (low, high, sl, _sh) = sender_sees_off(Feature::Files);
    let src = low.home.join("a.txt");
    std::fs::write(&src, b"a").unwrap();

    assert_feature_off(&sl.send_file(&high.id, &src), Feature::Files);
    assert!(asks(&high).is_empty());
}

#[test]
fn handoff_off_on_the_peer_refuses_the_handoff() {
    let (_low, high, sl, _sh) = sender_sees_off(Feature::Handoff);

    let url = HandoffItem::Url("https://example.com/".to_owned());
    assert_feature_off(&sl.handoff(&high.id, url), Feature::Handoff);
    assert!(asks(&high).is_empty());
}

#[test]
fn ring_off_on_the_peer_refuses_the_ring() {
    let (_low, high, sl, _sh) = sender_sees_off(Feature::Ring);

    assert_feature_off(&sl.ring(&high.id), Feature::Ring);
    assert!(high.platform.done().is_empty(), "nothing rang");
}

/// High runs with `off` turned off for low (clipboard stays on, so the
/// link is allowed); low dials it outside any supervisor.
fn receiver_with_off(off: &[Feature]) -> (Device, Device, Supervisor, Connection) {
    let (low, high, secret) = pair(&PolicyDoc::phone_defaults());
    let mut doc = PolicyDoc::phone_defaults();
    for f in off {
        doc.features.insert(*f, FeatureState::Off);
    }
    high.set_policy(&low.id, &doc);
    let sh = high.start(fast());
    let conn = low.dial(&high, &sh, &secret);
    high.wait("linked", link_up(&low.id));
    (low, high, sh, conn)
}

fn offer() -> FileOffer {
    FileOffer {
        id: 7,
        name: "run.sh".to_owned(),
        size: 4,
        mime: None,
    }
}

fn offer_evil(conn: &Connection) -> Result<(), LinkError> {
    let mut src = std::io::Cursor::new(b"evil".to_vec());
    offer_file(conn, &offer(), &mut src, &mut |_| {})
}

/// Nothing reached the user and the link is still up.
fn assert_untouched(low: &Device, high: &Device, sh: &Supervisor, conn: &Connection) {
    assert!(asks(high).is_empty(), "never asked");
    assert!(high.platform.done().is_empty(), "nothing opened or rang");
    assert!(carrova_link::ping(conn).is_ok());
    assert_eq!(sh.status()[0].state, PeerState::Linked);
    assert_eq!(high.count(link_down(&low.id)), 0);
}

fn assert_refused<T: std::fmt::Debug>(sent: &Result<T, LinkError>) {
    assert!(
        matches!(sent, Err(LinkError::Refused(RefuseReason::NotAllowed))),
        "{sent:?}"
    );
}

#[test]
fn receiver_refuses_a_file_lane_with_files_and_handoff_off() {
    let (low, high, sh, conn) = receiver_with_off(&[Feature::Files, Feature::Handoff]);

    assert_refused(&offer_evil(&conn));
    assert_untouched(&low, &high, &sh, &conn);
    assert!(!high.home.join("Downloads/run.sh").exists());
}

#[test]
fn receiver_refuses_a_handoff_lane_with_handoff_off() {
    let (low, high, sh, conn) = receiver_with_off(&[Feature::Handoff]);

    let url = Handoff::Url("https://example.com/".to_owned());
    assert_refused(&send_handoff(&conn, &url));
    assert_untouched(&low, &high, &sh, &conn);
}

#[test]
fn receiver_refuses_a_ring_lane_with_ring_off() {
    let (low, high, sh, conn) = receiver_with_off(&[Feature::Ring]);

    assert_refused(&send_ring(&conn, RingMsg::Start { seconds: 30 }));
    assert_untouched(&low, &high, &sh, &conn);
}

#[test]
fn plain_file_is_declined_unasked_when_only_handoff_is_on() {
    let (low, high, sh, conn) = receiver_with_off(&[Feature::Files]);

    // The lane is admitted (a handed-off file uses it), the file is not.
    assert!(matches!(offer_evil(&conn), Err(LinkError::Declined)));
    assert_untouched(&low, &high, &sh, &conn);
    assert!(!high.home.join("Downloads/run.sh").exists());
}
