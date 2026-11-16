//! One supervisor per user: the first `attach_or_start` runs it and serves
//! `<state>/link.sock`; later ones become clients of it.

use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::thread;

use carrova_core::ecosystem::{
    Answer, CallView, ClipSettings, ControlError, ControlRequest, ControlResponse,
    MAX_CONTROL_MESSAGE, SeenPeer, SharedSupervisor, Supervisor, SupervisorHandle, attach_or_start,
    attach_or_start_with,
};
use carrova_core::{CoreError, Engine, Event};
use carrova_link::{ClipItem, PolicyDoc};
use carrova_platform::{ClipContent, ClipKind, Platform, PlatformError};

use crate::supervisor::device::{Device, T, fast, pair, wait_until};

/// A second engine (another process) of `device`'s user.
fn other_process(device: &Device) -> Engine {
    Engine::new(device.platform.clone(), Vec::new(), device.sink.clone())
}

fn socket(device: &Device) -> PathBuf {
    device.platform.state_dir().unwrap().join("link.sock")
}

/// Whether another process can attach to `device`'s supervisor; tests
/// that need one skip (saying so) where the OS has no local sockets.
fn attachable(device: &Device) -> bool {
    let probe = device.platform.connect_local_socket(&socket(device));
    let unsupported = matches!(probe, Err(PlatformError::Unsupported(_)));
    if unsupported {
        eprintln!("skipped: this OS has no local sockets to attach over");
    }
    !unsupported
}

#[test]
fn second_supervisor_becomes_client() {
    let (low, _high, _) = pair(&PolicyDoc::phone_defaults());
    if !attachable(&low) {
        return;
    }

    let first = attach_or_start(low.engine.clone()).unwrap();
    let second = attach_or_start(other_process(&low)).unwrap();

    assert!(matches!(first, SupervisorHandle::Local(_)));
    assert!(matches!(second, SupervisorHandle::Remote(_)));
}

#[test]
fn client_status_matches_server() {
    let (low, high, _) = pair(&PolicyDoc::phone_defaults());
    if !attachable(&low) {
        return;
    }
    let server = attach_or_start(low.engine.clone()).unwrap();
    let client = attach_or_start(other_process(&low)).unwrap();

    let SupervisorHandle::Local(local) = &server else {
        panic!("first is not local")
    };
    let remote = client.status().unwrap();

    assert_eq!(remote, local.supervisor().status());
    assert_eq!(remote.len(), 1);
    assert_eq!(remote[0].id, high.id);
}

#[test]
fn stale_socket_file_replaced() {
    let (low, _high, _) = pair(&PolicyDoc::phone_defaults());
    if !attachable(&low) {
        return;
    }
    let path = socket(&low);
    // A socket left behind by a process that died: nobody listens on it.
    low.platform.bind_local_socket(&path).unwrap();
    assert!(path.exists());

    let server = attach_or_start(low.engine.clone()).unwrap();
    let client = attach_or_start(other_process(&low)).unwrap();

    assert!(matches!(server, SupervisorHandle::Local(_)));
    assert_eq!(client.status().unwrap().len(), 1);
}

#[test]
fn attach_takes_over_once_an_exiting_host_lets_go() {
    let (low, _high, _) = pair(&PolicyDoc::phone_defaults());
    // A host on its way out: it still holds the lock, its socket is gone.
    let dir = low.platform.state_dir().unwrap();
    let lock = low
        .platform
        .try_lock_exclusive(&dir.join("link.lock"))
        .unwrap()
        .unwrap();
    let exiting = thread::spawn(move || {
        thread::sleep(std::time::Duration::from_millis(300));
        drop(lock);
    });

    let started = std::time::Instant::now();
    let handle = attach_or_start(low.engine.clone()).unwrap();

    assert!(matches!(handle, SupervisorHandle::Local(_)));
    assert!(started.elapsed() < std::time::Duration::from_secs(2));
    exiting.join().unwrap();
}

/// One framed message read off `raw`.
fn read_reply(raw: &mut impl Read) -> ControlResponse {
    let mut len = [0; 4];
    raw.read_exact(&mut len).unwrap();
    let mut body = vec![0; u32::from_be_bytes(len) as usize];
    raw.read_exact(&mut body).unwrap();
    ciborium::from_reader(body.as_slice()).unwrap()
}

#[test]
fn unreadable_request_error_does_not_echo_it() {
    let (low, _high, _) = pair(&PolicyDoc::phone_defaults());
    if !attachable(&low) {
        return;
    }
    let _server = attach_or_start(low.engine.clone()).unwrap();
    let mut raw = low.platform.connect_local_socket(&socket(&low)).unwrap();
    let mut body = Vec::new();
    ciborium::into_writer("secret clip text", &mut body).unwrap();

    raw.write_all(&u32::try_from(body.len()).unwrap().to_be_bytes())
        .unwrap();
    raw.write_all(&body).unwrap();

    let ControlResponse::Failed(reason) = read_reply(&mut raw) else {
        panic!("not refused")
    };
    assert!(!reason.contains("secret"), "{reason}");
}

#[test]
fn oversize_request_rejected() {
    let (low, _high, _) = pair(&PolicyDoc::phone_defaults());
    if !attachable(&low) {
        return;
    }
    let _server = attach_or_start(low.engine.clone()).unwrap();
    let mut raw = low.platform.connect_local_socket(&socket(&low)).unwrap();

    raw.write_all(&(MAX_CONTROL_MESSAGE + 1).to_be_bytes())
        .unwrap();
    let reply = read_reply(&mut raw);

    assert!(matches!(reply, ControlResponse::Failed(_)), "{reply:?}");
    // The connection is closed, and the server still serves others.
    assert_eq!(raw.read(&mut [0; 4]).unwrap(), 0);
    let client = attach_or_start(other_process(&low)).unwrap();
    assert_eq!(client.status().unwrap().len(), 1);
}

#[test]
fn request_debug_hides_the_path() {
    let (low, _high, _) = pair(&PolicyDoc::phone_defaults());
    let request = ControlRequest::SendFile(low.id, PathBuf::from("/home/me/secret-plan.pdf"));

    let shown = format!("{request:?}");

    assert!(!shown.contains("secret-plan"), "{shown}");
    assert!(shown.starts_with("SendFile"), "{shown}");
}

#[test]
fn request_debug_hides_shared_text() {
    let (low, _high, _) = pair(&PolicyDoc::phone_defaults());
    let request = ControlRequest::SendText(low.id, "hunter2".to_owned());

    let shown = format!("{request:?}");

    assert!(!shown.contains("hunter2"), "{shown}");
}

#[test]
fn dropped_server_frees_the_lock_despite_an_idle_client() {
    let (low, _high, _) = pair(&PolicyDoc::phone_defaults());
    if !attachable(&low) {
        return;
    }
    let server = attach_or_start(low.engine.clone()).unwrap();
    let idle = attach_or_start(other_process(&low)).unwrap();
    // Served once, then idle: its connection is open on the server.
    assert_eq!(idle.status().unwrap().len(), 1);

    drop(server);

    let next = attach_or_start(other_process(&low)).unwrap();
    assert!(matches!(next, SupervisorHandle::Local(_)));
}

#[test]
fn runs_unserved_where_local_sockets_are_unsupported() {
    let (low, high, _) = pair(&PolicyDoc::phone_defaults());
    low.platform.refuse_local_sockets();

    let handle = attach_or_start(low.engine.clone()).unwrap();

    let SupervisorHandle::Local(server) = &handle else {
        panic!("not local")
    };
    assert_eq!(server.supervisor().status()[0].id, high.id);
    assert!(!socket(&low).exists());
}

/// Low runs the user's supervisor (served), a second process of low is its
/// client, and high runs its own; the two are linked.
/// Fields drop in order: everything running before the devices' dirs.
struct Served {
    server: SupervisorHandle,
    client: SupervisorHandle,
    events: Receiver<Event>,
    sh: Supervisor,
    low: Device,
    high: Device,
}

/// [`Served`], or `None` where the OS has no local sockets.
fn served_and_linked() -> Option<Served> {
    let (low, high, _) = pair(&PolicyDoc::phone_defaults());
    if !attachable(&low) {
        return None;
    }
    let server = attach_or_start_with(low.engine.clone(), low.source.clone(), fast()).unwrap();
    let client = attach_or_start(other_process(&low)).unwrap();
    assert!(matches!(client, SupervisorHandle::Remote(_)));
    let events = client.subscribe().unwrap();
    let sh = high.start(fast());
    let SupervisorHandle::Local(local) = &server else {
        panic!("not local")
    };
    low.sees(&high, &sh);
    high.sees(&low, local.supervisor());
    // Linked, and each side holds the other's policy (features gate on it).
    let has_policy = |peers: Vec<carrova_core::ecosystem::LinkedPeer>| {
        peers.first().is_some_and(|p| p.remote.is_some())
    };
    wait_until("policies", || {
        has_policy(sh.status()) && has_policy(server.status().unwrap())
    });
    Some(Served {
        low,
        high,
        server,
        client,
        events,
        sh,
    })
}

/// The first event `events` delivers that `f` picks.
fn next<R>(events: &Receiver<Event>, what: &str, f: impl Fn(Event) -> Option<R>) -> R {
    let deadline = std::time::Instant::now() + T;
    loop {
        let left = deadline.saturating_duration_since(std::time::Instant::now());
        let event = events
            .recv_timeout(left)
            .unwrap_or_else(|e| panic!("no {what}: {e}"));
        if let Some(r) = f(event) {
            return r;
        }
    }
}

#[test]
fn remote_subscriber_receives_link_up() {
    let Some(s) = served_and_linked() else { return };

    let up = next(&s.events, "link up", |e| match e {
        Event::LinkUp { device } => Some(device),
        _ => None,
    });

    assert_eq!(up, s.high.id);
}

#[test]
fn remote_client_answers_an_ask() {
    let Some(s) = served_and_linked() else { return };
    let src = s.high.home.join("notes.txt");
    std::fs::write(&src, b"remote answer").unwrap();
    next(&s.events, "link up", |e| {
        matches!(e, Event::LinkUp { .. }).then_some(())
    });

    thread::scope(|scope| {
        let sending = scope.spawn(|| s.sh.send_file(&s.low.id, &src));
        let ask = next(&s.events, "the ask", |e| match e {
            Event::AskReceived { ask, .. } => Some(ask),
            _ => None,
        });
        s.client.answer(ask, Answer::Accept).unwrap();
        sending.join().unwrap().unwrap();
    });

    let saved = s.low.home.join("Downloads/notes.txt");
    assert_eq!(std::fs::read(saved).unwrap(), b"remote answer");
}

#[test]
fn remote_client_answers_while_its_own_send_waits() {
    let Some(s) = served_and_linked() else { return };
    let out = s.low.home.join("out.txt");
    let inbound = s.high.home.join("in.txt");
    std::fs::write(&out, b"waiting").unwrap();
    std::fs::write(&inbound, b"answered").unwrap();
    next(&s.events, "link up", |e| {
        matches!(e, Event::LinkUp { .. }).then_some(())
    });

    thread::scope(|scope| {
        // Waits for high's answer, which comes only at the end.
        let waiting = scope.spawn(|| s.client.send_file(&s.high.id, &out));
        let outgoing = s.high.wait("the ask on high", |e| {
            matches!(e, Event::AskReceived { .. })
        });
        let sending = scope.spawn(|| s.sh.send_file(&s.low.id, &inbound));
        let ask = next(&s.events, "the ask on low", |e| match e {
            Event::AskReceived { ask, .. } => Some(ask),
            _ => None,
        });
        s.client.answer(ask, Answer::Accept).unwrap();
        sending.join().unwrap().unwrap();
        assert!(!waiting.is_finished(), "the send was still waiting");

        let Event::AskReceived { ask, .. } = outgoing else {
            unreachable!()
        };
        s.sh.answer(ask, Answer::Decline).unwrap();
        assert!(waiting.join().unwrap().is_err(), "declined");
    });

    let saved = s.low.home.join("Downloads/in.txt");
    assert_eq!(std::fs::read(saved).unwrap(), b"answered");
}

#[test]
fn remote_client_sends_text_up_to_the_clip_cap() {
    let Some(s) = served_and_linked() else { return };
    let text = "y".repeat(carrova_link::MAX_CLIP_TEXT);

    s.client.send_text(&s.high.id, text.clone()).unwrap();

    s.high
        .wait("clip", |e| matches!(e, Event::ClipReceived { .. }));
    let got = s.high.platform.clipboard_read().unwrap().unwrap().bytes;
    assert!(got == text.into_bytes(), "the whole text arrived");
}

#[test]
fn clip_history_round_trips() {
    let Some(s) = served_and_linked() else { return };
    next(&s.events, "link up", |e| {
        matches!(e, Event::LinkUp { .. }).then_some(())
    });
    let clip = ClipContent {
        kind: ClipKind::Text,
        bytes: b"from high".to_vec(),
        sensitive: false,
    };
    s.high.platform.clipboard_write(&clip).unwrap();
    wait_until("the clip", || !s.server.clip_history().unwrap().is_empty());

    let history = s.client.clip_history().unwrap();
    let items: Vec<_> = history.iter().map(|clip| &clip.item).collect();
    assert_eq!(items, [&ClipItem::Text("from high".to_owned())]);
    // Stamped on arrival.
    assert!(history[0].at >= carrova_core::time::now_unix() - 60);
    s.client.clear_clip_history().unwrap();
    assert!(s.server.clip_history().unwrap().is_empty());
}

#[test]
fn clip_history_of_max_size_pngs_round_trips() {
    let Some(s) = served_and_linked() else { return };
    let png = |n: u8| {
        // 0xff bytes: the most a byte costs in CBOR (two).
        let mut bytes = vec![0xff; carrova_link::MAX_CLIP_PNG];
        bytes[..8].copy_from_slice(b"\x89PNG\r\n\x1a\n");
        bytes[8] = n;
        bytes
    };
    for n in 1..=3 {
        let clip = ClipContent {
            kind: ClipKind::Png,
            bytes: png(n),
            sensitive: false,
        };
        s.high.platform.clipboard_write(&clip).unwrap();
        wait_until("the clip", || {
            s.server.clip_history().unwrap().len() == usize::from(n)
        });
    }

    let history: Vec<_> = s
        .client
        .clip_history()
        .unwrap()
        .into_iter()
        .map(|clip| clip.item)
        .collect();

    let expected: Vec<_> = (1..=3).rev().map(|n| ClipItem::Png(png(n))).collect();
    assert!(history == expected, "history differs");
}

/// Events into a channel.
struct ToChannel(std::sync::Mutex<std::sync::mpsc::Sender<Event>>);

impl carrova_core::EventSink for ToChannel {
    fn emit(&self, event: Event) {
        let _ = self.0.lock().unwrap().send(event);
    }
}

#[test]
fn shared_supervisor_attaches_again_when_its_host_exits() {
    let (low, high, _) = pair(&PolicyDoc::phone_defaults());
    if !attachable(&low) {
        return;
    }
    // Fake source: a real browse holds the host's drop ~2 s, long enough for
    // the forwarder to re-attach to the dying host and miss the link.
    let host = attach_or_start_with(low.engine.clone(), low.source.clone(), fast()).unwrap();
    let shared = std::sync::Arc::new(
        SharedSupervisor::attach_with(other_process(&low), low.source.clone(), fast()).unwrap(),
    );
    assert!(
        shared
            .call(|h| Ok(matches!(h, SupervisorHandle::Remote(_))))
            .unwrap()
    );
    let (tx, events) = std::sync::mpsc::channel();
    let forwarding = std::sync::Arc::clone(&shared);
    thread::spawn(move || forwarding.forward(&ToChannel(std::sync::Mutex::new(tx))));

    drop(host);

    // Requests work again: this process hosts it now.
    assert_eq!(shared.call(SupervisorHandle::status).unwrap().len(), 1);
    let port = shared
        .call(|h| match h {
            SupervisorHandle::Local(server) => Ok(server.supervisor().port()),
            SupervisorHandle::Remote(_) => panic!("still a client"),
        })
        .unwrap();
    // And so do events: high links with the new host.
    let sh = high.start(fast());
    low.sees(&high, &sh);
    high.source.set(vec![SeenPeer {
        id: low.id,
        addr: std::net::SocketAddrV4::new(std::net::Ipv4Addr::LOCALHOST, port),
    }]);
    let up = next(&events, "link up", |e| match e {
        Event::LinkUp { device } => Some(device),
        _ => None,
    });
    assert_eq!(up, high.id);
}

#[test]
fn clip_settings_round_trip_through_a_client() {
    let (low, _high, _) = pair(&PolicyDoc::phone_defaults());
    if !attachable(&low) {
        return;
    }
    let server = attach_or_start(low.engine.clone()).unwrap();
    let client = attach_or_start(other_process(&low)).unwrap();
    assert_eq!(client.clip_settings().unwrap(), ClipSettings::default());

    let changed = ClipSettings {
        share: false,
        show_note: true,
        history_file: false,
    };
    client.set_clip_settings(changed).unwrap();

    assert_eq!(server.clip_settings().unwrap(), changed);
    assert_eq!(client.clip_settings().unwrap(), changed);
}

#[test]
fn retrying_an_unknown_file_is_refused_through_a_client() {
    let (low, _high, _) = pair(&PolicyDoc::phone_defaults());
    if !attachable(&low) {
        return;
    }
    let _server = attach_or_start(low.engine.clone()).unwrap();
    let client = attach_or_start(other_process(&low)).unwrap();

    let refused = client.retry_file(42);

    assert!(
        matches!(refused, Err(CoreError::Control(ControlError::Failed(_)))),
        "{refused:?}"
    );
}

/// `value` through CBOR, as the control socket carries it.
fn cbor<T: serde::Serialize + serde::de::DeserializeOwned>(value: &T) -> T {
    let mut bytes = Vec::new();
    ciborium::into_writer(value, &mut bytes).unwrap();
    ciborium::from_reader(bytes.as_slice()).unwrap()
}

#[test]
fn control_wire_roundtrip_phase2() {
    use carrova_core::ecosystem::{PhoneSettings, RingHere};
    use carrova_link::{
        ActionResult, CallCommand, CallReply, Feature, NotifyAction, SendOutcome, SmsReply,
        SmsRequest,
    };
    use carrova_platform::phone::{CallState, Note, SmsMessage};

    let device = Device::new("Phone").id;
    let settings = PhoneSettings {
        ring_here: RingHere::Always,
        only_while_active: true,
        show_when_locked: false,
    };
    let note = Note {
        key: "k1".into(),
        app_id: "org.example.chat".into(),
        app_name: "Chat".into(),
        title: "Ana".into(),
        text: "see you at 6".into(),
        when_ms: 1,
        can_reply: true,
        icon: Some(vec![0x89, b'P', b'N', b'G']),
        hidden: None,
    };
    let message = SmsMessage {
        id: 0,
        thread_id: 5,
        outgoing: false,
        text: "on my way".into(),
        when_ms: 2,
    };
    let requests = [
        ControlRequest::Notes(device),
        ControlRequest::NoteAction(device, NotifyAction::Dismiss { key: "k1".into() }),
        ControlRequest::Sms(
            device,
            SmsRequest::Send {
                to: "5550100".into(),
                text: "hi".into(),
                client_id: 7,
            },
        ),
        ControlRequest::Call(device, CallCommand::Mute(true)),
        ControlRequest::PhoneSettings,
        ControlRequest::SetPhoneSettings(settings),
    ];
    for request in requests {
        assert_eq!(cbor(&request), request);
    }
    let responses = [
        ControlResponse::Notes(vec![note.clone()]),
        ControlResponse::NoteAction(ActionResult::Gone),
        ControlResponse::Sms(SmsReply::SendResult {
            client_id: 7,
            outcome: SendOutcome::Unknown,
        }),
        ControlResponse::Call(CallReply::NotRinging),
        ControlResponse::PhoneSettings(settings),
    ];
    for response in responses {
        assert_eq!(cbor(&response), response);
    }
    let events = [
        Event::PhoneNote { device, note },
        Event::PhoneNoteGone {
            device,
            key: "k1".to_owned().into(),
        },
        Event::PhoneSms { device, message },
        Event::PhoneCall {
            device,
            state: CallState::Ringing {
                number: "5550100".into(),
                name: Some("Ana".into()),
            },
        },
        Event::CallView(CallView::Ringing {
            device,
            number: "5550100".into(),
            name: None,
            ring_here: true,
        }),
        Event::CallView(CallView::None),
        Event::PhoneCleared {
            device,
            feature: Feature::Messages,
        },
    ];
    for event in events {
        assert_eq!(cbor(&event), event);
    }
}

#[test]
fn phone_requests_debug_hides_content() {
    let device = Device::new("Phone").id;
    let request = ControlRequest::Sms(
        device,
        carrova_link::SmsRequest::Send {
            to: "5550100".into(),
            text: "hunter2".into(),
            client_id: 7,
        },
    );

    let shown = format!("{request:?}");

    assert!(
        !shown.contains("5550100") && !shown.contains("hunter2"),
        "{shown}"
    );
}

#[test]
fn phone_events_debug_hides_note_keys() {
    let device = Device::new("Phone").id;
    let key = "0|org.example.secret|1|null|10123".to_owned();
    let gone = Event::PhoneNoteGone {
        device,
        key: key.clone().into(),
    };

    let shown = format!("{gone:?}");

    assert!(!shown.contains("secret"), "{shown}");
}
