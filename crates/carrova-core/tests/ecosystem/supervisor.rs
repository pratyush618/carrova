//! The link supervisor: two devices in one process over loopback, presence
//! replaced by a fake source, the platform recording what it opens.

mod clip_settings;
pub(crate) mod device;
mod gating;
mod recording;
mod retry;

use std::thread;

use carrova_core::ecosystem::{
    Answer, AskId, AskKind, ClipSkip, HandoffItem, PeerState, Supervisor, SupervisorConfig,
};
use carrova_core::{CoreError, Event};
use carrova_crypto::identity::DeviceId;
use carrova_link::{
    Feature, FeatureState, LaneGate, LaneKind, LinkError, PolicyDoc, RefuseReason, accept_lane,
    serve_ping,
};
use carrova_platform::{ClipContent, ClipKind, Platform};
use carrova_transport::Connection;
use serde::Serialize;

use device::{Device, T, fast, pair, start_both, wait_until};
use recording::Done;

fn link_up(device: &DeviceId) -> impl Fn(&Event) -> bool + '_ {
    move |e| matches!(e, Event::LinkUp { device: d } if d == device)
}

fn link_down(device: &DeviceId) -> impl Fn(&Event) -> bool + '_ {
    move |e| matches!(e, Event::LinkDown { device: d, .. } if d == device)
}

/// Wait until `sup` shows `peer`'s policy with `feature` in `state`.
fn wait_remote(sup: &Supervisor, peer: &DeviceId, feature: Feature, state: FeatureState) {
    wait_until("the peer's policy", || {
        sup.status().iter().any(|p| {
            p.id == *peer
                && p.state == PeerState::Linked
                && p.remote.as_ref().is_some_and(|r| r.get(feature) == state)
        })
    });
}

/// Both linked and each holding the other's policy.
fn linked(doc: &PolicyDoc) -> (Device, Device, Supervisor, Supervisor) {
    let (low, high, _) = pair(doc);
    let (sl, sh) = start_both(&low, &high);
    let clip = doc.get(Feature::Clipboard);
    wait_remote(&sl, &high.id, Feature::Clipboard, clip);
    wait_remote(&sh, &low.id, Feature::Clipboard, clip);
    (low, high, sl, sh)
}

fn text(s: &str) -> ClipContent {
    ClipContent {
        kind: ClipKind::Text,
        bytes: s.as_bytes().to_vec(),
        sensitive: false,
    }
}

fn clipboard(device: &Device) -> Option<Vec<u8>> {
    device.platform.clipboard_read().unwrap().map(|c| c.bytes)
}

#[test]
fn link_up_when_feature_on() {
    let (low, high, _) = pair(&PolicyDoc::phone_defaults());
    let (sl, sh) = start_both(&low, &high);

    low.wait("low linked", link_up(&high.id));
    high.wait("high linked", link_up(&low.id));
    let status = sl.status();
    assert_eq!(status.len(), 1);
    assert_eq!(status[0].id, high.id);
    assert_eq!(status[0].name, "High");
    assert_eq!(status[0].state, PeerState::Linked);
    assert_eq!(status[0].local, PolicyDoc::phone_defaults());
    wait_remote(&sh, &low.id, Feature::Ring, FeatureState::On);
    assert!(sl.ping(&high.id).unwrap() < T);
}

#[test]
fn presence_advertises_the_link_port_while_running() {
    use carrova_core::presence::{Visibility, advert};
    let (low, _high, _) = pair(&PolicyDoc::phone_defaults());
    let lp = || {
        advert(&*low.platform, Visibility::Known, 0)
            .unwrap()
            .unwrap()
            .link_port
    };
    assert_eq!(lp(), None);

    let sl = low.start(fast());
    assert_eq!(lp(), Some(sl.port()));

    sl.stop();
    assert_eq!(lp(), None);
}

#[test]
fn unreadable_policy_reads_as_all_off() {
    let (low, high, _) = pair(&PolicyDoc::phone_defaults());
    let state = low.platform.state_dir().unwrap();
    let file = state.join(format!("devices/policy/{}.json", high.id.to_hex()));
    std::fs::write(file, b"{ not json").unwrap();

    let sl = low.start(fast());

    let status = sl.status();
    assert_eq!(status.len(), 1);
    assert_eq!(status[0].local, PolicyDoc::computer_defaults());
}

#[test]
fn no_dial_when_all_off() {
    let (low, high, _) = pair(&PolicyDoc::computer_defaults());
    // Only the side that does not dial wants a feature.
    high.set_policy(&low.id, &PolicyDoc::phone_defaults());
    let (sl, sh) = start_both(&low, &high);

    low.source.wait_rounds(10);
    high.source.wait_rounds(10);

    assert_eq!(low.count(link_up(&high.id)), 0);
    assert_eq!(high.count(link_up(&low.id)), 0);
    assert_eq!(sl.status()[0].state, PeerState::Away);
    assert_eq!(sh.status()[0].state, PeerState::Away);
    assert!(matches!(
        sl.send_clipboard(&high.id),
        Err(CoreError::NotLinked)
    ));
}

/// Admits every lane.
struct Open;

impl LaneGate for Open {
    fn admit(&self, _: LaneKind) -> Result<(), RefuseReason> {
        Ok(())
    }
}

/// Serve lanes on `conn` until a ping arrived; `true` if one did.
fn serve_until_ping(conn: &Connection) -> bool {
    loop {
        match accept_lane(conn, &Open, T) {
            Ok((LaneKind::Ping, stream)) => return serve_ping(stream).is_ok(),
            Ok(_) | Err(LinkError::Refused(_)) => {}
            Err(_) => return false,
        }
    }
}

#[test]
fn duplicate_dial_keeps_one_connection() {
    let (low, high, secret) = pair(&PolicyDoc::phone_defaults());
    // Low redials its superseded link after `backoff_min`: long enough here
    // for the test's own connection to serve a ping first.
    let (sl, sh) = (
        low.start(SupervisorConfig {
            backoff_min: std::time::Duration::from_secs(1),
            backoff_max: std::time::Duration::from_secs(60),
            ..fast()
        }),
        high.start(fast()),
    );
    low.sees(&high, &sh);
    high.sees(&low, &sl);
    high.wait("high linked", link_up(&low.id));

    // The dialer's newer connection replaces the live one.
    let newer = low.dial(&high, &sh, &secret);
    let served = thread::spawn(move || (serve_until_ping(&newer), newer));
    wait_until("a ping over the newer connection", || {
        let _ = sh.ping(&low.id);
        served.is_finished()
    });

    let (pinged, _newer) = served.join().unwrap();
    assert!(pinged);
    let status = sh.status();
    assert_eq!(status.len(), 1);
    assert_eq!(status[0].state, PeerState::Linked);
    assert_eq!(high.count(link_up(&low.id)), 1);

    // Low's own link comes back once (it replaces the test's), then stays.
    wait_until("low relinked", || low.count(link_up(&high.id)) == 2);
    low.source.wait_rounds(20);
    assert_eq!(low.count(link_up(&high.id)), 2, "no redial loop");
    assert_eq!(high.count(link_up(&low.id)), 1);
    sl.ping(&high.id).unwrap();
}

#[test]
fn reconnect_after_drop() {
    let (low, high, _) = pair(&PolicyDoc::phone_defaults());
    let (sl, sh) = start_both(&low, &high);
    low.wait("first link", link_up(&high.id));

    sh.stop();
    let down = low.wait("link lost", link_down(&high.id));
    assert_eq!(
        down,
        Event::LinkDown {
            device: high.id,
            retrying: true
        }
    );

    let again = high.start(fast());
    low.sees(&high, &again);
    wait_until("second link", || low.count(link_up(&high.id)) == 2);
    sl.ping(&high.id).unwrap();
}

#[test]
fn forget_drops_live_link() {
    let (low, high, _) = pair(&PolicyDoc::phone_defaults());
    let (sl, sh) = start_both(&low, &high);
    low.wait("linked", link_up(&high.id));

    high.forget(&low.id);

    high.wait("high dropped the link", link_down(&low.id));
    low.wait("low lost the link", link_down(&high.id));
    // Low keeps trying (it still knows high) but is refused every time.
    low.source.wait_rounds(20);
    assert_eq!(low.count(link_up(&high.id)), 1);
    assert_ne!(sl.status()[0].state, PeerState::Linked);
    assert!(sh.status().is_empty());
}

#[test]
fn policy_change_applies_live() {
    let (low, high, sl, sh) = linked(&PolicyDoc::phone_defaults());

    sh.set_feature(&low.id, Feature::Clipboard, FeatureState::Off)
        .unwrap();
    wait_remote(&sl, &high.id, Feature::Clipboard, FeatureState::Off);
    low.platform.clipboard_write(&text("first")).unwrap();
    assert!(matches!(
        sl.send_clipboard(&high.id),
        Err(CoreError::FeatureOff(Feature::Clipboard))
    ));

    sh.set_feature(&low.id, Feature::Clipboard, FeatureState::On)
        .unwrap();
    wait_remote(&sl, &high.id, Feature::Clipboard, FeatureState::On);
    sl.send_clipboard(&high.id).unwrap();

    high.wait("clip", |e| matches!(e, Event::ClipReceived { .. }));
    assert_eq!(clipboard(&high).as_deref(), Some(&b"first"[..]));
    assert_eq!(low.count(link_up(&high.id)), 1);
    assert_eq!(
        PolicyStoreDoc::of(&high, &low.id).get(Feature::Clipboard),
        FeatureState::On
    );
}

/// The policy as stored on `device` for `peer`.
struct PolicyStoreDoc;

impl PolicyStoreDoc {
    fn of(device: &Device, peer: &DeviceId) -> PolicyDoc {
        carrova_core::ecosystem::PolicyStore::open(&*device.platform)
            .unwrap()
            .get(&*device.platform, peer)
            .unwrap()
    }
}

#[test]
fn clipboard_flows_only_when_both_on() {
    let mut doc = PolicyDoc::phone_defaults();
    let (low, high, _) = pair(&doc);
    doc.features.insert(Feature::Clipboard, FeatureState::Off);
    high.set_policy(&low.id, &doc);
    let (sl, sh) = start_both(&low, &high);
    wait_remote(&sl, &high.id, Feature::Clipboard, FeatureState::Off);

    low.platform.clipboard_write(&text("not shared")).unwrap();
    assert!(matches!(
        sl.send_clipboard(&high.id),
        Err(CoreError::FeatureOff(Feature::Clipboard))
    ));

    sh.set_feature(&low.id, Feature::Clipboard, FeatureState::On)
        .unwrap();
    wait_remote(&sl, &high.id, Feature::Clipboard, FeatureState::On);
    // Copied on low: sent by the clipboard watch, no explicit send.
    low.platform.clipboard_write(&text("shared")).unwrap();

    high.wait("clip", |e| matches!(e, Event::ClipReceived { .. }));
    assert_eq!(clipboard(&high).as_deref(), Some(&b"shared"[..]));
    low.wait("sent", |e| matches!(e, Event::ClipSent { .. }));
    // Received clips are not sent back (high's watch saw its own write):
    // the one clip low gets is the one high sends explicitly.
    sh.send_clipboard(&low.id).unwrap();
    low.wait("clip back", |e| matches!(e, Event::ClipReceived { .. }));
    assert_eq!(low.count(|e| matches!(e, Event::ClipReceived { .. })), 1);
    assert_eq!(high.count(|e| matches!(e, Event::ClipReceived { .. })), 1);
}

#[test]
fn shared_text_is_sent_as_a_clip() {
    let (low, high, sl, _sh) = linked(&PolicyDoc::phone_defaults());

    sl.send_text(&high.id, "shared words".to_owned()).unwrap();

    high.wait("clip", |e| matches!(e, Event::ClipReceived { .. }));
    assert_eq!(clipboard(&high).as_deref(), Some(&b"shared words"[..]));
    low.wait("sent", |e| matches!(e, Event::ClipSent { .. }));
    assert_eq!(
        clipboard(&low),
        None,
        "this device's clipboard is untouched"
    );
}

#[test]
fn shared_text_over_the_cap_is_skipped() {
    let (low, high, sl, _sh) = linked(&PolicyDoc::phone_defaults());
    let big = "x".repeat(carrova_link::MAX_CLIP_TEXT + 1);

    sl.send_text(&high.id, big).unwrap();

    low.wait("skipped", |e| {
        *e == Event::ClipSkipped {
            reason: ClipSkip::TooBig,
        }
    });
    assert_eq!(high.count(|e| matches!(e, Event::ClipReceived { .. })), 0);
}

#[test]
fn shared_text_needs_the_clipboard_feature() {
    let mut doc = PolicyDoc::phone_defaults();
    let (low, high, _) = pair(&doc);
    doc.features.insert(Feature::Clipboard, FeatureState::Off);
    high.set_policy(&low.id, &doc);
    let (sl, _sh) = start_both(&low, &high);
    wait_remote(&sl, &high.id, Feature::Clipboard, FeatureState::Off);

    assert!(matches!(
        sl.send_text(&high.id, "no".to_owned()),
        Err(CoreError::FeatureOff(Feature::Clipboard))
    ));
}

#[test]
fn sensitive_clip_skipped() {
    let (low, high, sl, _sh) = linked(&PolicyDoc::phone_defaults());

    let mut secret = text("hunter2");
    secret.sensitive = true;
    low.platform.clipboard_write(&secret).unwrap();
    low.wait("skipped", |e| {
        *e == Event::ClipSkipped {
            reason: ClipSkip::Sensitive,
        }
    });
    sl.send_clipboard(&high.id).unwrap();

    low.platform.clipboard_write(&text("plain")).unwrap();
    high.wait("clip", |e| matches!(e, Event::ClipReceived { .. }));
    assert_eq!(clipboard(&high).as_deref(), Some(&b"plain"[..]));
    assert_eq!(high.count(|e| matches!(e, Event::ClipReceived { .. })), 1);
    assert_eq!(
        low.count(|e| matches!(e, Event::ClipSkipped { .. })),
        2,
        "the watch and the explicit send both skip it"
    );
}

/// Every ask `device` received so far, oldest first.
fn asks(device: &Device) -> Vec<(AskId, DeviceId, AskKind)> {
    device
        .events()
        .into_iter()
        .filter_map(|e| match e {
            Event::AskReceived { ask, device, kind } => Some((ask, device, kind)),
            _ => None,
        })
        .collect()
}

#[test]
fn file_ask_accept_saves_to_downloads() {
    let (low, high, sl, sh) = linked(&PolicyDoc::phone_defaults());
    let src = low.home.join("report.txt");
    std::fs::write(&src, b"quarterly numbers").unwrap();

    // The second copy gets a new name instead of replacing the first.
    for (round, expected) in [(1, "report.txt"), (2, "report (2).txt")] {
        thread::scope(|s| {
            let sending = s.spawn(|| sl.send_file(&high.id, &src));
            wait_until("the ask", || asks(&high).len() == round);
            let (ask, device, kind) = asks(&high).pop().unwrap();
            assert_eq!(device, low.id);
            assert_eq!(
                kind,
                AskKind::File {
                    name: "report.txt".to_owned(),
                    size: 17
                }
            );
            sh.answer(ask, Answer::Accept).unwrap();
            sending.join().unwrap().unwrap();
        });
        let saved = high.home.join("Downloads").join(expected);
        assert_eq!(std::fs::read(&saved).unwrap(), b"quarterly numbers");
        high.wait(
            "saved",
            |e| matches!(e, Event::FileSaved { path, .. } if *path == saved),
        );
    }
    let downloads = std::fs::read_dir(high.home.join("Downloads")).unwrap();
    assert_eq!(downloads.count(), 2, "no partial file left behind");
}

#[test]
fn sender_hears_the_file_was_sent() {
    let (low, high, sl, sh) = linked(&PolicyDoc::phone_defaults());
    let src = low.home.join("empty.txt");
    std::fs::write(&src, b"").unwrap();

    thread::scope(|s| {
        let sending = s.spawn(|| sl.send_file(&high.id, &src));
        wait_until("the ask", || asks(&high).len() == 1);
        sh.answer(asks(&high)[0].0, Answer::Accept).unwrap();
        sending.join().unwrap().unwrap();
    });

    let Event::FileSaved { id, .. } = high.wait("saved", |e| matches!(e, Event::FileSaved { .. }))
    else {
        unreachable!()
    };
    let sent = Event::FileSent {
        device: high.id,
        id,
    };
    low.wait("sent", |e| *e == sent);
    assert_eq!(low.count(|e| matches!(e, Event::FileFailed { .. })), 0);
}

#[test]
fn stop_waits_for_lanes_in_flight() {
    let (low, high, sl, sh) = linked(&PolicyDoc::phone_defaults());
    let src = low.home.join("big.bin");
    std::fs::write(&src, vec![7; 1 << 20]).unwrap();

    thread::scope(|s| {
        let sending = s.spawn(|| sl.send_file(&high.id, &src));
        high.wait("ask", |e| matches!(e, Event::AskReceived { .. }));
        sh.stop();
        let at_stop = high.events().len();
        thread::sleep(std::time::Duration::from_millis(500));
        assert_eq!(high.events().len(), at_stop, "no event after stop returned");
        assert!(sending.join().unwrap().is_err());
    });
    let downloads = high.home.join("Downloads");
    let left = std::fs::read_dir(&downloads).map_or(0, Iterator::count);
    assert_eq!(left, 0, "nothing saved, no partial left");
}

#[test]
fn file_ask_expires_after_timeout() {
    let (low, high, _) = pair(&PolicyDoc::phone_defaults());
    let (sl, sh) = (
        low.start(fast()),
        high.start(SupervisorConfig {
            ask_expiry: std::time::Duration::from_millis(200),
            ..fast()
        }),
    );
    low.sees(&high, &sh);
    high.sees(&low, &sl);
    wait_remote(&sl, &high.id, Feature::Files, FeatureState::Ask);
    let src = low.home.join("late.bin");
    std::fs::write(&src, b"x").unwrap();

    let sent = sl.send_file(&high.id, &src);

    assert!(matches!(sent, Err(CoreError::Link(LinkError::Expired))));
    let Event::AskReceived { ask, .. } =
        high.wait("ask", |e| matches!(e, Event::AskReceived { .. }))
    else {
        unreachable!()
    };
    high.wait("expired", |e| *e == Event::AskExpired { ask });
    assert!(matches!(
        sh.answer(ask, Answer::Accept),
        Err(CoreError::AskGone)
    ));
    let downloads = high.home.join("Downloads");
    let left = std::fs::read_dir(&downloads).map_or(0, Iterator::count);
    assert_eq!(left, 0, "nothing saved, no partial left");
}

#[test]
fn handoff_url_opens_after_accept() {
    let (low, high, sl, sh) = linked(&PolicyDoc::phone_defaults());
    let url = "https://example.com/a?b=c";

    thread::scope(|s| {
        let sending = s.spawn(|| sl.handoff(&high.id, HandoffItem::Url(url.to_owned())));
        let Event::AskReceived { ask, kind, .. } =
            high.wait("ask", |e| matches!(e, Event::AskReceived { .. }))
        else {
            unreachable!()
        };
        assert_eq!(
            kind,
            AskKind::Handoff {
                url_host: Some("example.com".to_owned()),
                file: None
            }
        );
        assert!(
            high.platform.done().is_empty(),
            "nothing opens before the answer"
        );
        sh.answer(ask, Answer::Accept).unwrap();
        sending.join().unwrap().unwrap();
    });

    high.wait("opened", |e| *e == Event::HandoffOpened { device: low.id });
    assert_eq!(high.platform.done(), [Done::OpenUrl(url.to_owned())]);
}

#[test]
fn handoff_file_saves_then_opens_without_a_second_ask() {
    let (low, high, sl, sh) = linked(&PolicyDoc::phone_defaults());
    let src = low.home.join("slides.pdf");
    std::fs::write(&src, b"%PDF").unwrap();

    thread::scope(|s| {
        let sending = s.spawn(|| sl.handoff(&high.id, HandoffItem::File(src.clone())));
        let Event::AskReceived { ask, kind, .. } =
            high.wait("ask", |e| matches!(e, Event::AskReceived { .. }))
        else {
            unreachable!()
        };
        assert_eq!(
            kind,
            AskKind::Handoff {
                url_host: None,
                file: Some("slides.pdf".to_owned())
            }
        );
        sh.answer(ask, Answer::Accept).unwrap();
        sending.join().unwrap().unwrap();
    });

    let saved = high.home.join("Downloads").join("slides.pdf");
    high.wait("opened", |e| *e == Event::HandoffOpened { device: low.id });
    assert_eq!(std::fs::read(&saved).unwrap(), b"%PDF");
    assert_eq!(high.platform.done(), [Done::OpenPath(saved)]);
    assert_eq!(asks(&high).len(), 1);
}

#[test]
fn handoff_file_with_another_name_is_asked_again() {
    use carrova_link::{FileOffer, Handoff, OfferAnswer, offer_file, send_handoff};
    let (low, high, secret) = pair(&PolicyDoc::phone_defaults());
    let sh = high.start(fast());
    let conn = low.dial(&high, &sh, &secret);
    high.wait("linked", link_up(&low.id));
    let offer = |name: &str| FileOffer {
        id: 7,
        name: name.to_owned(),
        size: 4,
        mime: None,
    };

    thread::scope(|s| {
        let handing = s.spawn(|| send_handoff(&conn, &Handoff::File(offer("slides.pdf"))));
        wait_until("the handoff ask", || asks(&high).len() == 1);
        sh.answer(asks(&high)[0].0, Answer::Accept).unwrap();
        assert!(matches!(
            handing.join().unwrap(),
            Ok(OfferAnswer::Accept { .. })
        ));

        // Same id and size, another name: not the file the user accepted.
        let sending = s.spawn(|| {
            let mut src = std::io::Cursor::new(b"evil".to_vec());
            offer_file(&conn, &offer("run.sh"), &mut src, &mut |_| {})
        });
        wait_until("a second ask", || asks(&high).len() == 2);
        let (ask, _, kind) = asks(&high).pop().unwrap();
        assert_eq!(
            kind,
            AskKind::File {
                name: "run.sh".to_owned(),
                size: 4
            }
        );
        sh.answer(ask, Answer::Decline).unwrap();
        assert!(matches!(sending.join().unwrap(), Err(LinkError::Declined)));
    });
    assert!(high.platform.done().is_empty(), "nothing opened");
}

#[test]
fn ring_reaches_peer() {
    let (low, high, sl, _sh) = linked(&PolicyDoc::phone_defaults());

    sl.ring(&high.id).unwrap();

    high.wait("ringing", |e| *e == Event::RingStarted { device: low.id });
    assert_eq!(high.platform.done(), [Done::Ring(30)]);
    sl.stop_ring(&high.id).unwrap();
    high.wait("stopped", |e| *e == Event::RingStopped { device: low.id });
    assert_eq!(high.platform.done(), [Done::Ring(30), Done::StopRing]);
}

/// A lane opener from a future version.
#[derive(Serialize)]
struct FutureLane {
    kind: &'static str,
    version: u16,
}

#[test]
fn unknown_lane_does_not_drop_link() {
    let (low, high, secret) = pair(&PolicyDoc::phone_defaults());
    let sh = high.start(fast());
    let conn = low.dial(&high, &sh, &secret);
    high.wait("linked", link_up(&low.id));

    let mut stream = conn.open_stream().unwrap();
    stream
        .send(&FutureLane {
            kind: "Teleport",
            version: 1,
        })
        .unwrap();
    let reply: carrova_link::LaneReply = stream.recv(T).unwrap();

    assert_eq!(
        reply,
        carrova_link::LaneReply::Refused(RefuseReason::Unknown)
    );
    assert!(carrova_link::ping(&conn).is_ok());
    assert_eq!(sh.status()[0].state, PeerState::Linked);
    assert_eq!(high.count(link_down(&low.id)), 0);
}
