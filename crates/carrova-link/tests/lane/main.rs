//! Lane protocol and ping over two linked connections (public API).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[path = "../support/mod.rs"]
mod support;

use std::thread;

use carrova_link::{
    LaneGate, LaneKind, LaneReply, LinkError, RefuseReason, accept_lane, open_lane, ping,
    serve_ping,
};
use serde::Serialize;
use support::{Open, T, linked};

struct Gate(Result<(), RefuseReason>);

impl LaneGate for Gate {
    fn admit(&self, _: LaneKind) -> Result<(), RefuseReason> {
        self.0
    }
}

/// A `LaneOpen` as a newer peer might send it: any kind, any version.
#[derive(Serialize)]
struct RawOpen<V> {
    kind: &'static str,
    version: V,
}

/// The wire names are the protocol: renaming a variant breaks old peers.
#[test]
fn lane_kind_wire_names_are_pinned() {
    for (kind, name) in [
        (LaneKind::Policy, "Policy"),
        (LaneKind::Clipboard, "Clipboard"),
        (LaneKind::File, "File"),
        (LaneKind::Handoff, "Handoff"),
        (LaneKind::Ring, "Ring"),
        (LaneKind::Ping, "Ping"),
    ] {
        assert_eq!(serde_json::to_string(&kind).unwrap(), format!("\"{name}\""));
    }
}

#[test]
fn ping_roundtrip() {
    let (a, b) = linked();
    thread::scope(|s| {
        let server = s.spawn(|| {
            let (kind, stream) = accept_lane(&b, &Open, T).unwrap();
            assert_eq!(kind, LaneKind::Ping);
            serve_ping(stream).unwrap();
        });
        assert!(ping(&a).unwrap() < T);
        server.join().unwrap();
    });
}

/// Send `open` raw and return the acceptor's reply.
fn raw_reply<V: Serialize>(conn: &carrova_transport::Connection, open: &RawOpen<V>) -> LaneReply {
    let mut raw = conn.open_stream().unwrap();
    raw.send(open).unwrap();
    raw.recv(T).unwrap()
}

#[test]
fn unknown_lane_refused_connection_survives() {
    let (a, b) = linked();
    thread::scope(|s| {
        let server = s.spawn(|| {
            for _ in 0..5 {
                let refused = accept_lane(&b, &Open, T).map(|(k, _)| k);
                assert!(matches!(
                    refused,
                    Err(LinkError::Refused(RefuseReason::Unknown))
                ));
            }
            serve_ping(accept_lane(&b, &Open, T).unwrap().1).unwrap();
        });
        let unknown = LaneReply::Refused(RefuseReason::Unknown);
        assert_eq!(
            raw_reply(
                &a,
                &RawOpen {
                    kind: "Teleport",
                    version: 1
                }
            ),
            unknown
        );
        assert_eq!(
            raw_reply(
                &a,
                &RawOpen {
                    kind: "Ping",
                    version: 99
                }
            ),
            unknown
        );
        // Versions beyond u16, or not a number at all, are still a version mismatch.
        assert_eq!(
            raw_reply(
                &a,
                &RawOpen {
                    kind: "Ping",
                    version: 70_000_u32
                }
            ),
            unknown
        );
        assert_eq!(
            raw_reply(
                &a,
                &RawOpen {
                    kind: "Ping",
                    version: "one"
                }
            ),
            unknown
        );
        assert_eq!(
            raw_reply(
                &a,
                &RawOpen {
                    kind: "Ping",
                    version: -1
                }
            ),
            unknown
        );
        ping(&a).unwrap();
        server.join().unwrap();
    });
}

#[test]
fn silent_opener_is_dropped_within_the_accept_timeout() {
    let (a, b) = linked();
    let _silent = a.open_stream().unwrap();
    let start = std::time::Instant::now();
    let got = accept_lane(&b, &Open, std::time::Duration::from_millis(500)).map(|(k, _)| k);
    assert!(matches!(got, Err(LinkError::Transport(_))));
    assert!(start.elapsed() < std::time::Duration::from_secs(2));
}

#[test]
fn gate_refusal_reaches_opener() {
    let (a, b) = linked();
    thread::scope(|s| {
        let server = s.spawn(|| {
            let gate = Gate(Err(RefuseReason::NotAllowed));
            let got = accept_lane(&b, &gate, T).map(|(k, _)| k);
            assert!(matches!(
                got,
                Err(LinkError::Refused(RefuseReason::NotAllowed))
            ));
        });
        let got = open_lane(&a, LaneKind::Clipboard).map(|_| ());
        assert!(matches!(
            got,
            Err(LinkError::Refused(RefuseReason::NotAllowed))
        ));
        server.join().unwrap();
    });
}
