//! Public-API tests for the SMS lane.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[path = "../support/mod.rs"]
mod support;

use std::thread;

use carrova_link::{
    LaneKind, LinkError, MAX_SMS_TEXT, MESSAGE_PAGE, SendOutcome, SmsIncoming, SmsPush, SmsReply,
    SmsRequest, THREAD_PAGE, accept_lane, push_sms, recv_sms, request_sms,
};
use carrova_platform::phone::{SmsMessage, SmsThread};
use support::{Open, T, linked};

fn thread_row(id: i64) -> SmsThread {
    SmsThread {
        id,
        name: Some("Ann".into()),
        address: "+15550100".into(),
        snippet: "hi".into(),
        when_ms: id,
        unread: 0,
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
fn ask(req: &SmsRequest, reply: SmsReply) -> (Result<SmsReply, LinkError>, SmsRequest) {
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
    let req = SmsRequest::Threads { before_ms: Some(9) };
    let page = SmsReply::ThreadsPage((0..THREAD_PAGE as i64).map(thread_row).collect());
    let (out, got) = ask(&req, page.clone());
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
            // The phone's own side refuses to send it ...
            assert!(matches!(resp.reply(big), Err(LinkError::TooBig { .. })));
        });
        // ... and the lane closes without a reply.
        let req = SmsRequest::Thread {
            id: 2,
            before_ms: None,
        };
        assert!(request_sms(&a, &req).is_err());
        phone.join().unwrap();
    });
}

#[test]
fn send_result_roundtrip() {
    let req = SmsRequest::Send {
        to: "+1 555".replace(' ', ""),
        text: "hello".into(),
        client_id: 7,
    };
    let reply = SmsReply::SendResult {
        client_id: 7,
        outcome: SendOutcome::NoSignal,
    };
    let (out, got) = ask(&req, reply.clone());
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
            recv_sms(stream).unwrap()
        });
        push_sms(&phone, &push).unwrap();
        let SmsIncoming::Push(got) = rx.join().unwrap() else {
            panic!("not a push")
        };
        assert_eq!(got, push);
    });
}

#[test]
fn debug_hides_contents() {
    let push = SmsPush::Incoming(msg("secret".into()));
    let req = SmsRequest::Send {
        to: "+15550100".into(),
        text: "secret".into(),
        client_id: 1,
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
