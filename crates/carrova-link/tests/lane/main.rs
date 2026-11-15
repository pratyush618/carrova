//! Lane protocol and ping over two linked connections (public API).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[path = "../support/mod.rs"]
mod support;

use std::thread;

use carrova_link::{
    LaneGate, LaneKind, LinkError, RefuseReason, accept_lane, open_lane, ping, serve_ping,
};
use serde::Serialize;
use support::{T, linked};

struct Gate(Result<(), RefuseReason>);

impl LaneGate for Gate {
    fn admit(&self, _: LaneKind) -> Result<(), RefuseReason> {
        self.0
    }
}

const OPEN: Gate = Gate(Ok(()));

/// A `LaneOpen` as a newer peer might send it: any kind, any version.
#[derive(Serialize)]
struct RawOpen {
    kind: &'static str,
    version: u16,
}

#[test]
fn ping_roundtrip() {
    let (a, b) = linked();
    thread::scope(|s| {
        let server = s.spawn(|| {
            let (kind, stream) = accept_lane(&b, &OPEN, T).unwrap();
            assert_eq!(kind, LaneKind::Ping);
            serve_ping(stream).unwrap();
        });
        assert!(ping(&a).unwrap() < T);
        server.join().unwrap();
    });
}

#[test]
fn unknown_lane_refused_connection_survives() {
    let (a, b) = linked();
    thread::scope(|s| {
        let server = s.spawn(|| {
            for _ in 0..2 {
                let refused = accept_lane(&b, &OPEN, T).map(|(k, _)| k);
                assert!(matches!(
                    refused,
                    Err(LinkError::Refused(RefuseReason::Unknown))
                ));
            }
            serve_ping(accept_lane(&b, &OPEN, T).unwrap().1).unwrap();
        });
        for open in [
            RawOpen {
                kind: "Teleport",
                version: 1,
            },
            RawOpen {
                kind: "Ping",
                version: 99,
            },
        ] {
            let mut raw = a.open_stream().unwrap();
            raw.send(&open).unwrap();
            let reply: carrova_link::LaneReply = raw.recv(T).unwrap();
            assert_eq!(
                reply,
                carrova_link::LaneReply::Refused(RefuseReason::Unknown)
            );
        }
        ping(&a).unwrap();
        server.join().unwrap();
    });
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
