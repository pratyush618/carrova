//! Public-API tests for the one-request-one-reply lane shape.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[path = "../support/mod.rs"]
mod support;

use std::thread;

use support::{Open, T, linked};

use carrova_link::{LaneKind, accept_lane, request, serve};

#[test]
fn request_gets_the_servers_reply() {
    let (a, b) = linked();
    thread::scope(|s| {
        let server = s.spawn(|| {
            let (kind, stream) = accept_lane(&b, &Open, T).unwrap();
            assert_eq!(kind, LaneKind::Ping);
            let (asked, responder) = serve::<u32, String>(stream).unwrap();
            responder.reply(&format!("got {asked}")).unwrap();
        });
        let answer: String = request(&a, LaneKind::Ping, &7_u32, T).unwrap();
        server.join().unwrap();
        assert_eq!(answer, "got 7");
    });
}
