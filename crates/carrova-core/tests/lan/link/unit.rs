use std::net::{Ipv4Addr, SocketAddrV4};
use std::sync::Mutex;
use std::thread;

use carrova_crypto::pairing::PairingCode;
use carrova_transport::{ListenOptions, Listener, ProtocolViolation, Ticket};

use super::*;

/// Records events.
#[derive(Default)]
struct Events(Mutex<Vec<Event>>);

impl EventSink for Events {
    fn emit(&self, event: Event) {
        self.0.lock().unwrap().push(event);
    }
}

/// Fails the first dials with errors a stranger (spoofed discovery
/// reply, garbled or failed handshake) can cause, then reaches the
/// sender.
#[derive(Debug)]
struct Flaky {
    failures: Vec<TransportError>,
    then: Option<Connection>,
    dials: Arc<Mutex<u32>>,
}

impl Dialer for Flaky {
    fn resume(&mut self, _: &ResumeSecret, _: Duration) -> Result<Connection, TransportError> {
        *self.dials.lock().unwrap() += 1;
        match self.failures.pop() {
            Some(e) => Err(e),
            None => self.then.take().ok_or(TransportError::NotFound),
        }
    }
}

/// A paired connection pair over loopback: (sender side, receiver side).
fn paired() -> (Connection, Connection) {
    let mut listener = Listener::bind(ListenOptions {
        discovery: false,
        ..ListenOptions::default()
    })
    .unwrap();
    let code = PairingCode::generate().unwrap();
    listener.advertise_pairing(&code).unwrap();
    let ticket = Ticket {
        code: code.clone(),
        addr: Some(SocketAddrV4::new(Ipv4Addr::LOCALHOST, listener.port())),
    };
    let sender = thread::spawn(move || {
        let conn = listener
            .accept_pairing(&code, Duration::from_secs(30))
            .unwrap();
        (listener, conn)
    });
    let receiver = connect(
        Target::Pairing(&ticket),
        Duration::from_secs(30),
        &ConnectOptions::default(),
    )
    .unwrap();
    let (_listener, sender) = sender.join().unwrap();
    (sender, receiver)
}

#[test]
fn dial_errors_before_authentication_are_retried_until_the_sender_answers() {
    let (mut sender, receiver) = paired();
    let secret = receiver.resume_secret().clone();
    let dials = Arc::new(Mutex::new(0));
    let flaky = Flaky {
        failures: vec![
            TransportError::PairingFailed,
            ProtocolViolation::Malformed("garbage from a stranger".into()).into(),
        ],
        then: Some(receiver),
        dials: dials.clone(),
    };
    let mut link = Link::detached(secret, Box::new(flaky), Cancel::default());
    let events = Events::default();

    link.reconnect(
        ReceiverStage::Transferring,
        Duration::from_secs(60),
        &events,
    )
    .unwrap();

    assert_eq!(*dials.lock().unwrap(), 3);
    let first: LanMsg = sender.recv(Duration::from_secs(10)).unwrap();
    assert_eq!(
        first,
        LanMsg::Resume {
            stage: ReceiverStage::Transferring
        }
    );
    let events = events.0.lock().unwrap();
    assert!(matches!(events[..], [Event::PeerConnected { .. }]));
}

#[test]
fn a_sender_never_reached_is_unreachable_not_a_session_failure() {
    let (_sender, receiver) = paired();
    let flaky = Flaky {
        failures: vec![ProtocolViolation::Malformed("spoofed".into()).into()],
        then: None,
        dials: Arc::default(),
    };
    let mut link = Link::detached(
        receiver.resume_secret().clone(),
        Box::new(flaky),
        Cancel::default(),
    );
    let err = link
        .reconnect(
            ReceiverStage::Planning,
            Duration::from_millis(1500),
            &Events::default(),
        )
        .unwrap_err();
    assert!(matches!(err, CoreError::PeerUnreachable { .. }), "{err}");
}

#[test]
fn a_cancel_stops_reconnecting_at_once() {
    let (_sender, receiver) = paired();
    let flaky = Flaky {
        failures: Vec::new(),
        then: None,
        dials: Arc::default(),
    };
    let cancel = Cancel::default();
    let mut link = Link::detached(
        receiver.resume_secret().clone(),
        Box::new(flaky),
        cancel.clone(),
    );
    cancel.cancel();
    let started = Instant::now();
    let err = link
        .reconnect(
            ReceiverStage::Transferring,
            Duration::from_secs(600),
            &Events::default(),
        )
        .unwrap_err();
    assert!(matches!(err, CoreError::Cancelled), "{err}");
    assert!(started.elapsed() < Duration::from_secs(5));
}
