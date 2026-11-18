//! Public-API tests for the SMS lane.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[path = "../support/mod.rs"]
mod support;

use std::thread;

use serde::{Deserialize, Serialize};

use carrova_link::{
    LaneKind, LinkError, MAX_SIMS, MAX_SMS_TEXT, MESSAGE_PAGE, SendOutcome, SmsIncoming, SmsPush,
    SmsReply, SmsRequest, THREAD_PAGE, accept_lane, check_sms_reply, open_lane, push_sms, recv_sms,
    request_sms,
};
use carrova_platform::phone::{Cursor, Sim, SmsMessage, SmsThread};
use support::{Open, T, linked};

fn thread_row(id: i64) -> SmsThread {
    SmsThread {
        id,
        name: Some("Ann".into()),
        address: "+15550100".into(),
        snippet: "hi".into(),
        when_ms: id,
        unread: 0,
        sim: Some(2),
    }
}

fn msg(text: String) -> SmsMessage {
    SmsMessage {
        id: 1,
        thread_id: 2,
        outgoing: false,
        text,
        when_ms: 3,
    }
}

/// Phone side: answer one request with `reply`.
fn ask(req: &SmsRequest, reply: &SmsReply) -> (Result<SmsReply, LinkError>, SmsRequest) {
    let (a, b) = linked();
    thread::scope(|s| {
        let phone = s.spawn(|| {
            let (kind, stream) = accept_lane(&b, &Open, T).unwrap();
            assert_eq!(kind, LaneKind::Messages);
            let SmsIncoming::Request(got, resp) = recv_sms(stream).unwrap() else {
                panic!("not a request")
            };
            let _ = resp.reply(reply);
            got
        });
        let out = request_sms(&a, req);
        (out, phone.join().unwrap())
    })
}

#[test]
fn threads_page_roundtrip() {
    let req = SmsRequest::Threads {
        before: Some(Cursor { when_ms: 9, id: 4 }),
    };
    let page = SmsReply::ThreadsPage((0..THREAD_PAGE as i64).map(thread_row).collect());
    let (out, got) = ask(&req, &page);
    assert_eq!(got, req);
    assert_eq!(out.unwrap(), page);
}

#[test]
fn page_over_cap_refused() {
    let (a, b) = linked();
    let big = SmsReply::ThreadPage((0..=MESSAGE_PAGE).map(|_| msg("x".into())).collect());
    thread::scope(|s| {
        let phone = s.spawn(|| {
            let (_, stream) = accept_lane(&b, &Open, T).unwrap();
            let SmsIncoming::Request(_, resp) = recv_sms(stream).unwrap() else {
                panic!("not a request")
            };
            // The phone's own check catches it ...
            assert!(matches!(
                check_sms_reply(&big),
                Err(LinkError::TooBig { .. })
            ));
            drop(resp.reply(&big));
        });
        // ... and the laptop refuses it too.
        let req = SmsRequest::Thread {
            id: 2,
            before: None,
        };
        assert!(matches!(
            request_sms(&a, &req),
            Err(LinkError::TooBig { .. })
        ));
        phone.join().unwrap();
    });
}

#[test]
fn send_result_roundtrip() {
    let req = SmsRequest::Send {
        to: "+1 555".replace(' ', ""),
        text: "hello".into(),
        client_id: 7,
        sim: None,
    };
    let reply = SmsReply::SendResult {
        client_id: 7,
        outcome: SendOutcome::NoSignal,
    };
    let (out, got) = ask(&req, &reply);
    assert_eq!(got, req);
    assert_eq!(out.unwrap(), reply);
}

#[test]
fn send_text_over_cap_refused() {
    let (a, _b) = linked();
    let req = SmsRequest::Send {
        to: "+1555".into(),
        text: "x".repeat(MAX_SMS_TEXT + 1),
        client_id: 1,
        sim: None,
    };
    assert!(matches!(
        request_sms(&a, &req),
        Err(LinkError::TooBig { .. })
    ));
}

#[test]
fn implausible_number_refused() {
    let (a, _b) = linked();
    let req = SmsRequest::Send {
        to: "call me; rm".into(),
        text: "x".into(),
        client_id: 1,
        sim: None,
    };
    assert!(matches!(
        request_sms(&a, &req),
        Err(LinkError::Protocol("number"))
    ));
}

#[test]
fn incoming_push_roundtrip() {
    let (phone, laptop) = linked();
    let push = SmsPush::Incoming(msg("secret".into()));
    thread::scope(|s| {
        let rx = s.spawn(|| {
            let (kind, stream) = accept_lane(&laptop, &Open, T).unwrap();
            assert_eq!(kind, LaneKind::Messages);
            let incoming = recv_sms(stream).unwrap();
            let shown = format!("{incoming:?}");
            assert!(!shown.contains("secret"), "{shown}");
            let SmsIncoming::Push(got, ack) = incoming else {
                panic!("not a push")
            };
            ack.ack().unwrap();
            got
        });
        push_sms(&phone, &push).unwrap();
        let got = rx.join().unwrap();
        assert_eq!(got, push);
    });
}

#[test]
fn resync_push_roundtrip() {
    let (phone, laptop) = linked();
    thread::scope(|s| {
        let rx = s.spawn(|| {
            let (_, stream) = accept_lane(&laptop, &Open, T).unwrap();
            let incoming = recv_sms(stream).unwrap();
            let shown = format!("{incoming:?}");
            assert!(!shown.contains("secret"), "{shown}");
            let SmsIncoming::Push(got, ack) = incoming else {
                panic!("not a push")
            };
            ack.ack().unwrap();
            got
        });
        push_sms(&phone, &SmsPush::Resync).unwrap();
        let got = rx.join().unwrap();
        assert_eq!(got, SmsPush::Resync);
    });
}

#[test]
fn stored_push_roundtrip() {
    let (phone, laptop) = linked();
    thread::scope(|s| {
        let rx = s.spawn(|| {
            let (_, stream) = accept_lane(&laptop, &Open, T).unwrap();
            let incoming = recv_sms(stream).unwrap();
            let shown = format!("{incoming:?}");
            assert!(!shown.contains("secret"), "{shown}");
            let SmsIncoming::Push(got, ack) = incoming else {
                panic!("not a push")
            };
            ack.ack().unwrap();
            got
        });
        push_sms(&phone, &SmsPush::Stored { thread_id: 7 }).unwrap();
        let got = rx.join().unwrap();
        assert_eq!(got, SmsPush::Stored { thread_id: 7 });
    });
}

#[test]
fn debug_hides_contents() {
    let push = SmsPush::Incoming(msg("secret".into()));
    let req = SmsRequest::Send {
        to: "+15550100".into(),
        text: "secret".into(),
        client_id: 1,
        sim: None,
    };
    let reply = SmsReply::ThreadsPage(vec![thread_row(1)]);
    for s in [
        format!("{push:?}"),
        format!("{req:?}"),
        format!("{reply:?}"),
    ] {
        assert!(!s.contains("secret") && !s.contains("5550100") && !s.contains("Ann"));
    }
}

/// Test-local twin of the private wire frame: same variant names, same bytes.
#[derive(Serialize, Deserialize)]
enum Frame {
    Request(SmsRequest),
    Push(SmsPush),
}

/// A phone that answers any request with `reply`, unchecked.
fn hostile_phone(req: &SmsRequest, reply: &SmsReply) -> Result<SmsReply, LinkError> {
    let (a, b) = linked();
    thread::scope(|s| {
        s.spawn(|| {
            let (_, mut stream) = accept_lane(&b, &Open, T).unwrap();
            let _: Frame = stream.recv(T).unwrap();
            stream.send(reply).unwrap();
            stream.finish().unwrap();
        });
        request_sms(&a, req)
    })
}

fn threads() -> SmsRequest {
    SmsRequest::Threads { before: None }
}

#[test]
fn laptop_refuses_hostile_replies() {
    let big_page = SmsReply::ThreadsPage((0..=THREAD_PAGE as i64).map(thread_row).collect());
    assert!(matches!(
        hostile_phone(&threads(), &big_page),
        Err(LinkError::TooBig { .. })
    ));
    let big_text = SmsReply::ThreadPage(vec![msg("x".repeat(MAX_SMS_TEXT + 1))]);
    let ask = SmsRequest::Thread {
        id: 2,
        before: None,
    };
    assert!(matches!(
        hostile_phone(&ask, &big_text),
        Err(LinkError::TooBig { .. })
    ));
    let mut long = thread_row(1);
    long.address = "9".repeat(257);
    assert!(matches!(
        hostile_phone(&threads(), &SmsReply::ThreadsPage(vec![long])),
        Err(LinkError::TooBig { .. })
    ));
    let mut long = thread_row(1);
    long.snippet = "x".repeat(MAX_SMS_TEXT + 1);
    assert!(matches!(
        hostile_phone(&threads(), &SmsReply::ThreadsPage(vec![long])),
        Err(LinkError::TooBig { .. })
    ));
}

#[test]
fn reply_must_match_request() {
    let send = SmsRequest::Send {
        to: "+1555".into(),
        text: "x".into(),
        client_id: 7,
        sim: None,
    };
    let wrong_id = SmsReply::SendResult {
        client_id: 8,
        outcome: SendOutcome::Sent,
    };
    for (req, reply) in [
        (threads(), SmsReply::ThreadPage(vec![])),
        (
            SmsRequest::Thread {
                id: 2,
                before: None,
            },
            SmsReply::ThreadsPage(vec![]),
        ),
        (send.clone(), SmsReply::ThreadsPage(vec![])),
        (send, wrong_id),
        (SmsRequest::Remind, SmsReply::ThreadsPage(vec![])),
        (threads(), SmsReply::Reminded { posted: true }),
    ] {
        assert!(matches!(
            hostile_phone(&req, &reply),
            Err(LinkError::Protocol("reply"))
        ));
    }
}

/// A laptop that opens a lane and sends `frame`; what the phone's `recv_sms` says.
fn hostile_laptop(frame: &Frame) -> Result<(), LinkError> {
    let (a, b) = linked();
    thread::scope(|s| {
        let phone = s.spawn(|| {
            let (_, stream) = accept_lane(&b, &Open, T).unwrap();
            recv_sms(stream).map(drop)
        });
        let mut stream = open_lane(&a, LaneKind::Messages).unwrap();
        stream.send(frame).unwrap();
        phone.join().unwrap()
    })
}

#[test]
fn phone_refuses_hostile_requests_and_pushes() {
    let send = |to: &str, text: String| {
        Frame::Request(SmsRequest::Send {
            to: to.into(),
            text,
            client_id: 1,
            sim: None,
        })
    };
    for bad in ["", "+1 555", "call;rm", &"1".repeat(33)] {
        assert!(matches!(
            hostile_laptop(&send(bad, "x".into())),
            Err(LinkError::Protocol("number"))
        ));
    }
    assert!(matches!(
        hostile_laptop(&send("+1555", "x".repeat(MAX_SMS_TEXT + 1))),
        Err(LinkError::TooBig { .. })
    ));
    let push = Frame::Push(SmsPush::Incoming(msg("x".repeat(MAX_SMS_TEXT + 1))));
    assert!(matches!(
        hostile_laptop(&push),
        Err(LinkError::TooBig { .. })
    ));
}

#[test]
fn access_push_roundtrip() {
    let (phone, laptop) = linked();
    thread::scope(|s| {
        let rx = s.spawn(|| {
            let (_, stream) = accept_lane(&laptop, &Open, T).unwrap();
            let SmsIncoming::Push(got, ack) = recv_sms(stream).unwrap() else {
                panic!("not a push")
            };
            ack.ack().unwrap();
            got
        });
        let push = SmsPush::Access { granted: false };
        push_sms(&phone, &push).unwrap();
        assert_eq!(rx.join().unwrap(), push);
    });
}

#[test]
fn remind_roundtrip() {
    let reply = SmsReply::Reminded { posted: true };
    let (out, got) = ask(&SmsRequest::Remind, &reply);
    assert_eq!(got, SmsRequest::Remind);
    assert_eq!(out.unwrap(), reply);
}

fn sim(id: i32, slot: u8) -> Sim {
    Sim {
        id,
        slot,
        name: "Jio".into(),
        sms_default: slot == 0,
        voice_default: slot == 0,
    }
}

#[test]
fn sims_roundtrip_and_send_names_one() {
    let reply = SmsReply::Sims(vec![sim(3, 0), sim(5, 1)]);
    let (out, got) = ask(&SmsRequest::Sims, &reply);
    assert_eq!(got, SmsRequest::Sims);
    assert_eq!(out.unwrap(), reply);
    let req = SmsRequest::Send {
        to: "+1555".into(),
        text: "x".into(),
        client_id: 9,
        sim: Some(5),
    };
    let result = SmsReply::SendResult {
        client_id: 9,
        outcome: SendOutcome::Sent,
    };
    let (_, got) = ask(&req, &result);
    assert_eq!(got, req);
    assert!(!format!("{reply:?}").contains("Jio"));
}

#[test]
fn laptop_refuses_oversized_sim_lists() {
    let many = SmsReply::Sims((0..=MAX_SIMS as i32).map(|i| sim(i, 0)).collect());
    assert!(matches!(
        hostile_phone(&SmsRequest::Sims, &many),
        Err(LinkError::TooBig { .. })
    ));
    let mut long = sim(1, 0);
    long.name = "x".repeat(257);
    assert!(matches!(
        hostile_phone(&SmsRequest::Sims, &SmsReply::Sims(vec![long])),
        Err(LinkError::TooBig { .. })
    ));
    assert!(matches!(
        hostile_phone(&SmsRequest::Sims, &SmsReply::ThreadsPage(vec![])),
        Err(LinkError::Protocol("reply"))
    ));
}

/// The request and thread shapes before SIMs, as an older build has them.
mod old {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    pub enum SmsRequest {
        Send {
            to: String,
            text: String,
            client_id: u64,
        },
    }

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    pub struct SmsThread {
        pub id: i64,
        pub name: Option<String>,
        pub address: String,
        pub snippet: String,
        pub when_ms: i64,
        pub unread: u32,
    }
}

fn cbor<T: Serialize>(value: &T) -> Vec<u8> {
    let mut bytes = Vec::new();
    ciborium::into_writer(value, &mut bytes).unwrap();
    bytes
}

#[test]
fn older_builds_interoperate() {
    // An older laptop's send: the phone's default SIM.
    let from_old = old::SmsRequest::Send {
        to: "+1555".into(),
        text: "x".into(),
        client_id: 4,
    };
    let got: SmsRequest = ciborium::from_reader(cbor(&from_old).as_slice()).unwrap();
    assert_eq!(
        got,
        SmsRequest::Send {
            to: "+1555".into(),
            text: "x".into(),
            client_id: 4,
            sim: None,
        }
    );
    // An older phone reads a send naming a SIM (it ignores the SIM).
    let to_old = SmsRequest::Send {
        to: "+1555".into(),
        text: "x".into(),
        client_id: 4,
        sim: Some(2),
    };
    let got: old::SmsRequest = ciborium::from_reader(cbor(&to_old).as_slice()).unwrap();
    assert_eq!(got, from_old);
    // ... but can't read a SIM list request: the lane fails, so no choice shows.
    assert!(
        ciborium::from_reader::<old::SmsRequest, _>(cbor(&SmsRequest::Sims).as_slice()).is_err()
    );
    // Threads cross both ways; an older phone's know no SIM.
    let old_row: old::SmsThread = ciborium::from_reader(cbor(&thread_row(3)).as_slice()).unwrap();
    assert_eq!(old_row.id, 3);
    let row: SmsThread = ciborium::from_reader(cbor(&old_row).as_slice()).unwrap();
    assert_eq!(row.sim, None);
}
