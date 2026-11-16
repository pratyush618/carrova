//! The phone's forwarder: a supervisor on the phone, linked to test
//! laptops that dial it outside any supervisor and record what arrives.

use std::sync::{Arc, Mutex};
use std::thread;

use carrova_core::ecosystem::{NotifyApps, PeerState, Supervisor};
use carrova_crypto::pairing::KnownSecret;
use carrova_link::{
    ActionResult, CallCommand, CallIncoming, CallReply, Feature, FeatureState, Incoming, LaneGate,
    LaneKind, LinkError, NotifyAction, NotifyMsg, PolicyDoc, RefuseReason, SmsIncoming, SmsPush,
    SmsReply, SmsRequest, accept_lane, call_command, open_lane, recv_calls, recv_notify,
    recv_policy, recv_sms, request_sms, send_action, send_policy,
};
use carrova_platform::phone::{CallState, Note, PhoneEvent, SendOutcome, SmsMessage, SmsThread};
use carrova_transport::Connection;

use crate::supervisor::device::{Device, T, fast, pair, wait_until};

/// What a test laptop received from the phone.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Got {
    Notify(NotifyMsg),
    Sms(SmsPush),
    Call(CallState),
}

/// A laptop linked to the phone, recording what the phone sends.
struct Laptop {
    device: Device,
    conn: Arc<Connection>,
    got: Arc<Mutex<Vec<Got>>>,
}

impl Laptop {
    fn got(&self) -> Vec<Got> {
        self.got.lock().unwrap().clone()
    }

    /// Wait until `n` items arrived; returns them.
    fn wait_got(&self, n: usize) -> Vec<Got> {
        wait_until("phone updates", || self.got.lock().unwrap().len() >= n);
        self.got()
    }
}

/// Admits every lane.
struct Open;

impl LaneGate for Open {
    fn admit(&self, _: LaneKind) -> Result<(), RefuseReason> {
        Ok(())
    }
}

fn doc(on: &[Feature]) -> PolicyDoc {
    let mut doc = PolicyDoc::computer_defaults();
    for &f in on {
        doc.features.insert(f, FeatureState::On);
    }
    doc
}

/// Dial `phone` as `device`, which allows the phone `laptop_doc`; returns
/// once the phone holds that policy.
fn link(
    phone: &Device,
    sp: &Supervisor,
    device: Device,
    secret: &KnownSecret,
    laptop_doc: &PolicyDoc,
) -> Laptop {
    let conn = Arc::new(device.dial(phone, sp, secret));
    let got = Arc::new(Mutex::new(Vec::new()));
    let (c, g) = (Arc::clone(&conn), Arc::clone(&got));
    thread::spawn(move || {
        while let Ok((kind, stream)) = accept_lane(&c, &Open, T) {
            let item = match kind {
                LaneKind::Policy => {
                    let mut stream = stream;
                    let _ = recv_policy(&mut stream, T);
                    None
                }
                LaneKind::Notify => match recv_notify(stream) {
                    Ok(Incoming::Msg(m)) => Some(Got::Notify(m)),
                    _ => None,
                },
                LaneKind::Messages => match recv_sms(stream) {
                    Ok(SmsIncoming::Push(p)) => Some(Got::Sms(p)),
                    _ => None,
                },
                LaneKind::Calls => match recv_calls(stream) {
                    Ok(CallIncoming::State(s)) => Some(Got::Call(s)),
                    _ => None,
                },
                _ => None,
            };
            g.lock().unwrap().extend(item);
        }
    });
    let mut lane = open_lane(&conn, LaneKind::Policy).unwrap();
    send_policy(&mut lane, laptop_doc).unwrap();
    let id = device.id;
    wait_until("the phone has the laptop's policy", || {
        sp.status()
            .iter()
            .any(|p| p.id == id && p.state == PeerState::Linked && p.remote.is_some())
    });
    Laptop { device, conn, got }
}

/// A phone allowing `phone_on` (beyond clipboard) and a laptop allowing
/// `laptop_on`, linked; the phone's scripted data is set by `setup` first.
fn phone_and_laptop(
    phone_on: &[Feature],
    laptop_on: &[Feature],
    setup: impl FnOnce(&Device),
) -> (Device, Supervisor, Laptop) {
    let (laptop, phone, secret) = pair(&PolicyDoc::computer_defaults());
    let mut phone_doc = doc(phone_on);
    phone_doc
        .features
        .insert(Feature::Clipboard, FeatureState::On);
    phone.set_policy(&laptop.id, &phone_doc);
    setup(&phone);
    let sp = phone.start(fast());
    let laptop = link(&phone, &sp, laptop, &secret, &doc(laptop_on));
    (phone, sp, laptop)
}

fn note(key: &str, app_id: &str) -> Note {
    Note {
        key: key.into(),
        app_id: app_id.into(),
        app_name: "App".into(),
        title: "Ana".into(),
        text: "see you at 6".into(),
        when_ms: 1,
        can_reply: true,
        icon: None,
        hidden: None,
    }
}

fn ringing() -> CallState {
    CallState::Ringing {
        number: "5550100".into(),
        name: None,
    }
}

fn wait_watching(phone: &Device) {
    wait_until("the phone's events watched", || phone.platform.watching());
}

const NOTES: Feature = Feature::Notifications;
const CALLS: Feature = Feature::Calls;
const SMS: Feature = Feature::Messages;

#[test]
fn snapshot_on_link_open() {
    let (_phone, _sp, laptop) = phone_and_laptop(&[NOTES, CALLS], &[NOTES, CALLS], |p| {
        let mut phone = p.platform.phone();
        phone.notes = vec![
            note("k1", "org.example.chat"),
            note("k2", "com.snapwork.hdfc"),
        ];
        phone.call = ringing();
    });

    let got = laptop.wait_got(2);
    assert_eq!(
        got,
        [
            Got::Notify(NotifyMsg::Snapshot(vec![note("k1", "org.example.chat")])),
            Got::Call(ringing()),
        ]
    );
}

#[test]
fn watch_starts_before_the_snapshot() {
    // A note posted between the snapshot and the watch would be lost.
    let (phone, _sp, laptop) = phone_and_laptop(&[NOTES], &[NOTES], |_| {});

    laptop.wait_got(1);
    let actions = phone.platform.actions();
    let at = |name: &str| actions.iter().position(|a| a == name).unwrap();
    assert!(at("watch_phone_events") < at("active_notes"), "{actions:?}");
}

#[test]
fn app_off_not_forwarded() {
    let (phone, _sp, laptop) = phone_and_laptop(&[NOTES], &[NOTES], |p| {
        let mut apps = NotifyApps::open(&*p.platform).unwrap();
        apps.set(&*p.platform, "org.example.muted", false).unwrap();
    });
    wait_watching(&phone);

    phone
        .platform
        .script_event(PhoneEvent::NotePosted(note("m", "org.example.muted")));
    phone
        .platform
        .script_event(PhoneEvent::NotePosted(note("c", "org.example.chat")));
    phone
        .platform
        .script_event(PhoneEvent::NoteRemoved { key: "m".into() });
    phone
        .platform
        .script_event(PhoneEvent::NoteRemoved { key: "c".into() });

    let got = laptop.wait_got(3);
    assert_eq!(
        got,
        [
            Got::Notify(NotifyMsg::Snapshot(Vec::new())),
            Got::Notify(NotifyMsg::Posted(note("c", "org.example.chat"))),
            Got::Notify(NotifyMsg::Removed { key: "c".into() }),
        ]
    );
}

#[test]
fn phone_policy_off_blocks_even_if_laptop_on() {
    let (phone, _sp, laptop) = phone_and_laptop(&[CALLS], &[NOTES, CALLS], |p| {
        p.platform.phone().notes = vec![note("k1", "org.example.chat")];
    });
    wait_watching(&phone);

    phone
        .platform
        .script_event(PhoneEvent::NotePosted(note("k2", "org.example.chat")));
    phone.platform.script_event(PhoneEvent::Call(ringing()));

    // The call state on link open, then the ringing: no notification.
    let got = laptop.wait_got(2);
    assert_eq!(got, [Got::Call(CallState::Idle), Got::Call(ringing())]);
    let dismissed = send_action(&laptop.conn, &NotifyAction::Dismiss { key: "k1".into() });
    assert!(
        matches!(dismissed, Err(LinkError::Refused(RefuseReason::NotAllowed))),
        "{dismissed:?}"
    );
    assert!(
        !phone
            .platform
            .actions()
            .iter()
            .any(|a| a.starts_with("note_"))
    );
}

#[test]
fn reply_reaches_platform() {
    let (phone, _sp, laptop) = phone_and_laptop(&[NOTES], &[NOTES], |p| {
        p.platform.phone().notes = vec![note("k1", "org.example.chat")];
    });

    let reply = NotifyAction::Reply {
        key: "k1".into(),
        text: "on my way".into(),
    };
    assert_eq!(
        send_action(&laptop.conn, &reply).unwrap(),
        ActionResult::Done
    );
    assert!(
        phone
            .platform
            .actions()
            .contains(&"note_reply k1".to_owned())
    );
}

#[test]
fn reply_to_removed_note_reports_gone() {
    let (_phone, _sp, laptop) = phone_and_laptop(&[NOTES], &[NOTES], |_| {});

    let reply = NotifyAction::Reply {
        key: "k1".into(),
        text: "on my way".into(),
    };
    assert_eq!(
        send_action(&laptop.conn, &reply).unwrap(),
        ActionResult::Gone
    );
}

/// The phone actions from `call_answer` on.
fn after_answer(phone: &Device) -> Vec<String> {
    let actions = phone.platform.actions();
    let at = actions.iter().position(|a| a == "call_answer").unwrap();
    actions[at..].to_vec()
}

/// A phone ringing that, once answered, reports `answered`.
fn answering(answered: Option<CallState>) -> impl FnOnce(&Device) {
    move |p| {
        let mut phone = p.platform.phone();
        phone.call = ringing();
        phone.answered = answered;
    }
}

fn active(on_laptop_audio: bool) -> CallState {
    CallState::Active {
        since_ms: 1,
        muted: false,
        on_laptop_audio,
    }
}

#[test]
fn answer_turns_speaker_on() {
    let (phone, _sp, laptop) = phone_and_laptop(&[CALLS], &[CALLS], answering(Some(active(false))));

    assert_eq!(
        call_command(&laptop.conn, &CallCommand::Answer).unwrap(),
        CallReply::Done
    );
    assert!(after_answer(&phone).contains(&"call_speaker true".to_owned()));
}

#[test]
fn answer_with_laptop_audio_keeps_speaker_off() {
    let (phone, _sp, laptop) = phone_and_laptop(&[CALLS], &[CALLS], answering(Some(active(true))));

    assert_eq!(
        call_command(&laptop.conn, &CallCommand::Answer).unwrap(),
        CallReply::Done
    );
    assert!(
        !after_answer(&phone)
            .iter()
            .any(|a| a.starts_with("call_speaker"))
    );
}

#[test]
fn answer_still_ringing_leaves_speaker_alone() {
    // The phone never reports the call active: where audio goes is unknown.
    let (phone, _sp, laptop) = phone_and_laptop(&[CALLS], &[CALLS], answering(None));

    assert_eq!(
        call_command(&laptop.conn, &CallCommand::Answer).unwrap(),
        CallReply::Done
    );
    assert!(
        !after_answer(&phone)
            .iter()
            .any(|a| a.starts_with("call_speaker"))
    );
}

#[test]
fn watch_stops_when_no_feature_on() {
    let (phone, sp, laptop) = phone_and_laptop(&[NOTES], &[NOTES], |_| {});
    wait_watching(&phone);

    sp.set_feature(&laptop.device.id, NOTES, FeatureState::Off)
        .unwrap();

    wait_until("the watch stopped", || !phone.platform.watching());
}

#[test]
fn dropped_event_resyncs_every_feature() {
    let (a, phone, secret) = pair(&PolicyDoc::computer_defaults());
    // A laptop with the lower key: the phone would close a link it should
    // have dialled itself, to a device presence never saw.
    let b = std::iter::repeat_with(|| Device::new("B"))
        .find(|b| b.id.as_bytes() < phone.id.as_bytes())
        .unwrap();
    let all = doc(&[NOTES, SMS, CALLS]);
    phone.set_policy(&a.id, &all);
    phone.know(&b, "B", &secret, &all);
    b.know(&phone, "Phone", &secret, &PolicyDoc::computer_defaults());
    phone.platform.phone().notes = vec![note("k1", "org.example.chat")];
    let sp = phone.start(fast());
    let a = link(&phone, &sp, a, &secret, &doc(&[NOTES, CALLS]));
    let b = link(&phone, &sp, b, &secret, &doc(&[SMS]));
    let snapshot = Got::Notify(NotifyMsg::Snapshot(vec![note("k1", "org.example.chat")]));
    assert_eq!(
        a.wait_got(2),
        [snapshot.clone(), Got::Call(CallState::Idle)]
    );
    wait_watching(&phone);
    let active = CallState::Active {
        since_ms: 1,
        muted: false,
        on_laptop_audio: false,
    };

    phone
        .platform
        .script_event(PhoneEvent::Call(active.clone()));
    wait_until("the active call", || a.got().len() == 3);
    // The call ended while events were lost: the stale `Active` is replaced.
    phone
        .platform
        .script_event(PhoneEvent::Dropped { count: 3 });

    assert_eq!(
        a.wait_got(5),
        [
            snapshot.clone(),
            Got::Call(CallState::Idle),
            Got::Call(active),
            snapshot,
            Got::Call(CallState::Idle),
        ]
    );
    assert_eq!(b.wait_got(1), [Got::Sms(SmsPush::Resync)]);
    // Nothing more trails in for either.
    phone.source.wait_rounds(5);
    assert_eq!(a.got().len(), 5);
    assert_eq!(b.got().len(), 1);
}

#[test]
fn send_with_same_client_id_sends_once() {
    let (phone, _sp, laptop) = phone_and_laptop(&[SMS], &[SMS], |_| {});
    let send = SmsRequest::Send {
        to: "5550100".into(),
        text: "on my way".into(),
        client_id: 7,
    };
    let sent = SmsReply::SendResult {
        client_id: 7,
        outcome: SendOutcome::Sent,
    };

    assert_eq!(request_sms(&laptop.conn, &send).unwrap(), sent);
    assert_eq!(request_sms(&laptop.conn, &send).unwrap(), sent);
    let sends = phone
        .platform
        .actions()
        .iter()
        .filter(|a| *a == "sms_send")
        .count();
    assert_eq!(sends, 1);
}

#[test]
fn sms_pages_stop_at_the_sync_window() {
    let now = i64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis(),
    )
    .unwrap();
    let old = now - 31 * 24 * 3600 * 1000;
    let (_phone, _sp, laptop) = phone_and_laptop(&[SMS], &[SMS], |p| {
        let mut phone = p.platform.phone();
        for (id, when_ms) in [(1, now), (2, old)] {
            phone.threads.push(SmsThread {
                id,
                name: None,
                address: "5550100".into(),
                snippet: String::new(),
                when_ms,
                unread: 0,
            });
            phone.messages.push(SmsMessage {
                id,
                thread_id: 1,
                outgoing: false,
                text: String::new(),
                when_ms,
            });
        }
    });

    let threads = request_sms(&laptop.conn, &SmsRequest::Threads { before: None }).unwrap();
    let SmsReply::ThreadsPage(threads) = threads else {
        panic!("not a threads page");
    };
    assert_eq!(threads.iter().map(|t| t.id).collect::<Vec<_>>(), [1]);
    let thread = SmsRequest::Thread {
        id: 1,
        before: None,
    };
    let SmsReply::ThreadPage(messages) = request_sms(&laptop.conn, &thread).unwrap() else {
        panic!("not a thread page");
    };
    assert_eq!(messages.iter().map(|m| m.id).collect::<Vec<_>>(), [1]);
}
