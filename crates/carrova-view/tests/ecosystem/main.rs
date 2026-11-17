//! Ecosystem view-models (public API).
#![allow(clippy::unwrap_used)]

use std::time::Duration;

use carrova_core::Event;
use carrova_core::Ticket;
use carrova_core::ecosystem::{
    AskId, AskKind, FileFailure, LinkedPeer, PairInvite, PeerFound, PeerState,
};
use carrova_crypto::identity::{DeviceKey, Fingerprint};
use carrova_crypto::pairing::PairingCode;
use carrova_link::{ClipItem, Feature, FeatureState, Group, PolicyDoc, allowed};
use carrova_platform::OsKind;
use carrova_view::ecosystem::{
    AskTopic, ClipKind, PREVIEW_CHARS, TransferState, Transfers, ask_view, clip_history_view,
    feature_rows, linked_device_views, pair_invite_view, pair_peer_view,
};
use carrova_view::lan::code_view;

fn doc(entries: &[(Feature, FeatureState)]) -> PolicyDoc {
    PolicyDoc {
        version: 1,
        features: entries.iter().copied().collect(),
    }
}

#[test]
fn rows_list_phase_two_features_in_spec_order() {
    let rows = feature_rows(&PolicyDoc::phone_defaults(), None);
    let order: Vec<_> = rows.iter().map(|r| r.feature).collect();
    assert_eq!(
        order,
        [
            Feature::Clipboard,
            Feature::Ring,
            Feature::Files,
            Feature::Handoff,
            Feature::Notifications,
            Feature::Messages,
            Feature::Calls
        ]
    );
    assert_eq!(rows[0].group, Group::Quiet);
    assert_eq!(rows[2].group, Group::Ask);
}

#[test]
fn rows_show_remote_off() {
    let local = PolicyDoc::phone_defaults();
    let remote = doc(&[(Feature::Clipboard, FeatureState::On)]);
    let rows = feature_rows(&local, Some(&remote));
    let ring = rows.iter().find(|r| r.feature == Feature::Ring).unwrap();
    assert_eq!(ring.local, FeatureState::On);
    assert_eq!(ring.remote, Some(FeatureState::Off));
    assert_eq!(ring.effective, FeatureState::Off);
}

#[test]
fn rows_without_a_remote_document_flow_nothing() {
    for row in feature_rows(&PolicyDoc::phone_defaults(), None) {
        assert_eq!(row.remote, None);
        assert_eq!(row.effective, FeatureState::Off, "{:?}", row.feature);
    }
}

#[test]
fn effective_matches_gate() {
    use FeatureState::{Ask, Off, On};
    let sides = [On, Ask, Off];
    for l in sides {
        for r in sides {
            let local = doc(&[(Feature::Clipboard, l), (Feature::Files, l)]);
            let remote = doc(&[(Feature::Clipboard, r), (Feature::Files, r)]);
            for row in feature_rows(&local, Some(&remote)) {
                let f = row.feature;
                assert_eq!(
                    row.effective,
                    allowed(&local, &remote, f),
                    "{f:?} {l:?}/{r:?}"
                );
            }
        }
    }
}

#[test]
fn invite_view_has_svg_and_uri() {
    let invite = PairInvite {
        ticket: Ticket {
            code: PairingCode::generate().unwrap(),
            addr: Some("192.168.1.20:4433".parse().unwrap()),
        },
        key: DeviceKey::generate().unwrap().public(),
    };
    let code = code_view(
        &invite.ticket,
        carrova_core::CodeOffer::First,
        Duration::from_secs(90),
    );
    let view = pair_invite_view(&invite, code).unwrap();
    assert_eq!(view.uri, invite.to_uri());
    assert!(view.qr_svg.contains("<svg"), "{}", view.qr_svg);
    assert!(view.qr_svg.len() > 1000 && view.qr_svg.len() < 50_000);
    assert_eq!(view.expires_in_s, 90);
    assert_eq!(view.code.code, invite.ticket.code.to_string());
}

#[test]
fn history_preview_truncates_120() {
    let long = "a".repeat(500);
    let view = clip_history_view([(&ClipItem::Text(long), 7)]);
    assert_eq!(view.items[0].preview.chars().count(), PREVIEW_CHARS);
    assert_eq!(view.items[0].kind, ClipKind::Text);
    assert_eq!(view.items[0].at, 7);
}

#[test]
fn history_preview_cuts_on_char_boundaries() {
    let multi = "é".repeat(PREVIEW_CHARS + 5);
    let view = clip_history_view([(&ClipItem::Text(multi), 0)]);
    assert_eq!(view.items[0].preview, "é".repeat(PREVIEW_CHARS));
}

#[test]
fn short_text_is_kept_whole_and_images_show_no_content() {
    let png = ClipItem::Png(vec![1, 2, 3]);
    let text = ClipItem::Text("hi".into());
    let view = clip_history_view([(&text, 2), (&png, 1)]);
    assert_eq!(view.items[0].preview, "hi");
    assert_eq!(view.items[1].kind, ClipKind::Image);
    assert!(view.items[1].preview.is_empty());
}

fn peer(seed: u8, state: PeerState, remote: Option<PolicyDoc>) -> LinkedPeer {
    LinkedPeer {
        id: DeviceKey::from_bytes(&[seed; 32]).public(),
        name: "Pixel".into(),
        os: OsKind::Android,
        state,
        local: PolicyDoc::phone_defaults(),
        remote,
        missing: Vec::new(),
    }
}

#[test]
fn linked_device_view_shows_identity_state_and_feature_rows() {
    let p = peer(3, PeerState::Away, None);
    let views = linked_device_views(std::slice::from_ref(&p));
    assert_eq!(views[0].id, p.id.to_hex());
    assert_eq!(
        views[0].fingerprint,
        carrova_crypto::identity::Fingerprint::of(&p.id).to_string()
    );
    assert_eq!(views[0].state, PeerState::Away);
    assert_eq!(views[0].os, OsKind::Android);
    assert_eq!(views[0].features, feature_rows(&p.local, None));
}

#[test]
fn linked_device_view_allows_only_what_the_gate_lets_through() {
    let remote = doc(&[
        (Feature::Clipboard, FeatureState::On),
        (Feature::Ring, FeatureState::Off),
    ]);
    let view = &linked_device_views(&[peer(4, PeerState::Linked, Some(remote))])[0];
    assert!(view.allows(Feature::Clipboard));
    assert!(!view.allows(Feature::Ring), "off on the other side");

    let unknown = &linked_device_views(&[peer(5, PeerState::Linked, None)])[0];
    assert!(!unknown.allows(Feature::Clipboard), "no remote policy yet");
}

#[test]
fn ask_view_for_a_file_has_name_and_size_only() {
    let kind = AskKind::File {
        name: "a.txt".into(),
        size: 9,
    };
    let v = ask_view(ask_id(), "Pixel", &kind, 55);
    assert_eq!(v.kind, AskTopic::File);
    assert_eq!(
        (v.file_name.as_deref(), v.size, v.url_host.clone()),
        (Some("a.txt"), Some(9), None)
    );
    assert_eq!((v.device_name.as_str(), v.expires_in_s), ("Pixel", 55));
}

#[test]
fn ask_view_for_a_handoff_link_has_host_only() {
    let kind = AskKind::Handoff {
        url_host: Some("example.org".into()),
        file: None,
    };
    let v = ask_view(ask_id(), "Pixel", &kind, 55);
    assert_eq!(v.kind, AskTopic::Handoff);
    assert_eq!(
        (v.url_host.as_deref(), v.file_name, v.size),
        (Some("example.org"), None, None)
    );
}

fn dev() -> carrova_crypto::identity::DevicePublic {
    DeviceKey::from_bytes(&[9; 32]).public()
}

#[test]
fn transfer_progress_then_saved_completes_it() {
    let mut t = Transfers::default();
    t.apply(
        &Event::FileProgress {
            device: dev(),
            id: 4,
            done: 10,
            total: 100,
        },
        "Pixel",
    );
    assert_eq!(t.views()[0].state, TransferState::Active);
    assert_eq!((t.views()[0].done, t.views()[0].total), (10, 100));
    t.apply(
        &Event::FileSaved {
            device: dev(),
            id: 4,
            path: "/d/a.txt".into(),
        },
        "Pixel",
    );
    let v = &t.views()[0];
    assert_eq!(
        (v.state.clone(), v.done, v.file_name.as_str()),
        (TransferState::Done, 100, "a.txt")
    );
    assert_eq!(t.views().len(), 1);
}

#[test]
fn outgoing_transfer_completes_when_sent() {
    let mut t = Transfers::default();
    let progress = Event::FileProgress {
        device: dev(),
        id: 5,
        done: 100,
        total: 100,
    };
    t.apply(&progress, "Pixel");
    t.apply(
        &Event::FileSent {
            device: dev(),
            id: 5,
        },
        "Pixel",
    );
    let v = &t.views()[0];
    assert_eq!((v.state.clone(), v.done), (TransferState::Done, 100));
    assert_eq!(t.views().len(), 1);
}

#[test]
fn transfer_failure_keeps_progress_reason_and_retry() {
    let mut t = Transfers::default();
    t.apply(
        &Event::FileProgress {
            device: dev(),
            id: 4,
            done: 10,
            total: 100,
        },
        "Pixel",
    );
    t.apply(
        &Event::FileFailed {
            device: dev(),
            id: 4,
            reason: FileFailure::DeviceLeft,
            retry: true,
        },
        "Pixel",
    );
    assert_eq!(
        t.views()[0].state,
        TransferState::Failed {
            reason: FileFailure::DeviceLeft,
            retry: true,
        }
    );
    assert_eq!(t.views()[0].done, 10);
}

#[test]
fn unrelated_events_leave_transfers_alone() {
    let mut t = Transfers::default();
    t.apply(&Event::RingStarted { device: dev() }, "Pixel");
    assert!(t.views().is_empty());
}

#[test]
fn debug_output_hides_the_pairing_code_and_uri() {
    let invite = PairInvite {
        ticket: Ticket {
            code: PairingCode::generate().unwrap(),
            addr: Some("192.168.1.20:4433".parse().unwrap()),
        },
        key: DeviceKey::generate().unwrap().public(),
    };
    let code = code_view(
        &invite.ticket,
        carrova_core::CodeOffer::First,
        Duration::from_secs(90),
    );
    let view = pair_invite_view(&invite, code.clone()).unwrap();
    for text in [format!("{view:?}"), format!("{code:?}")] {
        assert!(!text.contains(&invite.ticket.code.to_string()), "{text}");
        assert!(!text.contains("carrova://"), "{text}");
        assert!(!text.contains("192.168.1.20"), "{text}");
    }
}

/// `AskId` has no public constructor: it arrives in events, i.e. as JSON.
fn ask_id() -> AskId {
    serde_json::from_value(serde_json::json!(1)).unwrap()
}

#[test]
fn pair_peer_view_cleans_name_and_formats_fingerprint() {
    let public = DeviceKey::from_bytes(&[7; 32]).public();
    let found = PeerFound::new("\u{202e}Pixel\n 8", OsKind::Android, public);

    let view = pair_peer_view(&found);

    assert_eq!(view.name, "Pixel 8");
    assert_eq!(view.os, OsKind::Android);
    assert_eq!(view.fingerprint, Fingerprint::of(&public).to_string());
    assert!(view.fingerprint.contains('·'), "{}", view.fingerprint);
}

#[test]
fn linked_device_view_names_the_access_the_phone_lacks() {
    let mut p = peer(6, PeerState::Linked, None);
    p.missing = vec![Feature::Messages];
    assert_eq!(linked_device_views(&[p])[0].missing, [Feature::Messages]);
}
