//! Public-API tests for the calls lane.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[path = "../support/mod.rs"]
mod support;

use std::thread;

use carrova_link::{
    AudioSide, CallCommand, CallIncoming, CallReply, HISTORY_PAGE, LaneKind, LinkError, MAX_SIMS,
    accept_lane, call_command, open_lane, recv_calls, send_call_state,
};
use carrova_platform::phone::{CallKind, CallRecord, CallState, Cursor, Sim};
use support::{Open, T, linked};

#[test]
fn state_roundtrip() {
    let (a, b) = linked();
    let state = CallState::Ringing {
        number: "+15551234".into(),
        name: Some("Ada".into()),
    };
    let got = thread::scope(|s| {
        let server = s.spawn(|| {
            let (kind, stream) = accept_lane(&b, &Open, T).unwrap();
            assert_eq!(kind, LaneKind::Calls);
            let incoming = recv_calls(stream).unwrap();
            let shown = format!("{incoming:?}");
            assert!(
                !shown.contains("5551234") && !shown.contains("Ada"),
                "{shown}"
            );
            let CallIncoming::State(got, ack) = incoming else {
                panic!("not a state")
            };
            ack.ack().unwrap();
            got
        });
        send_call_state(&a, &state).unwrap();
        server.join().unwrap()
    });
    assert_eq!(got, state);
}

#[test]
fn dial_rejects_letters_and_long() {
    let (a, _b) = linked();
    for bad in ["", "call me", "555-1234", &"1".repeat(33)] {
        let dial = CallCommand::Dial {
            number: bad.into(),
            sim: None,
        };
        assert!(matches!(
            call_command(&a, &dial),
            Err(LinkError::Protocol("number"))
        ));
    }
}

/// Serve one command with `answer`, return what the client got and what the server saw.
fn command(cmd: &CallCommand, answer: &CallReply) -> (CallReply, CallCommand) {
    let (a, b) = linked();
    thread::scope(|s| {
        let server = s.spawn(|| {
            let (_, stream) = accept_lane(&b, &Open, T).unwrap();
            let CallIncoming::Command(seen, responder) = recv_calls(stream).unwrap() else {
                panic!("expected a command");
            };
            responder.reply(answer).unwrap();
            seen
        });
        let reply = call_command(&a, cmd).unwrap();
        (reply, server.join().unwrap())
    })
}

#[test]
fn history_page_roundtrip() {
    let cmd = CallCommand::History {
        before: Some(Cursor { when_ms: 99, id: 3 }),
    };
    let page = vec![CallRecord {
        id: 2,
        number: "+1555".into(),
        name: None,
        when_ms: 7,
        duration_s: 61,
        kind: CallKind::Missed,
    }];
    let (reply, seen) = command(&cmd, &CallReply::HistoryPage(page.clone()));
    assert_eq!(seen, cmd);
    assert_eq!(reply, CallReply::HistoryPage(page));
    assert_eq!(HISTORY_PAGE, 50);
}

#[test]
fn answer_when_idle_returns_not_ringing() {
    // The server double stands in for core: scripted state Idle.
    let (reply, seen) = command(&CallCommand::Answer, &CallReply::NotRinging);
    assert_eq!(seen, CallCommand::Answer);
    assert_eq!(reply, CallReply::NotRinging);
}

/// Put `command` on the wire bypassing the sender's checks, return what the receiver made of it.
fn recv_raw(command: CallCommand) -> Result<CallIncoming, LinkError> {
    #[derive(serde::Serialize)]
    enum CallFrame {
        Command(CallCommand),
    }
    let (a, b) = linked();
    thread::scope(|s| {
        let server = s.spawn(|| {
            let (_, stream) = accept_lane(&b, &Open, T).unwrap();
            recv_calls(stream)
        });
        let mut lane = open_lane(&a, LaneKind::Calls).unwrap();
        lane.send(&CallFrame::Command(command)).unwrap();
        server.join().unwrap()
    })
}

#[test]
fn receiver_rejects_a_bad_number() {
    let dial = CallCommand::Dial {
        number: "x".into(),
        sim: None,
    };
    assert!(matches!(recv_raw(dial), Err(LinkError::Protocol("number"))));
}

#[test]
fn receiver_rejects_laptop_local_commands() {
    for local in [
        CallCommand::Tones("123".into()),
        CallCommand::AudioTo(AudioSide::Phone),
    ] {
        assert!(matches!(recv_raw(local), Err(LinkError::Protocol("local"))));
    }
}

#[test]
fn mute_and_end_commands_roundtrip() {
    for cmd in [CallCommand::Mute(true), CallCommand::End] {
        let (reply, seen) = command(&cmd, &CallReply::Done);
        assert_eq!((reply, seen), (CallReply::Done, cmd));
    }
}

#[test]
fn debug_hides_numbers() {
    let dial = CallCommand::Dial {
        number: "5551234".into(),
        sim: Some(7_654_321),
    };
    let tones = CallCommand::Tones("5551234".into());
    let shown = format!("{dial:?}{tones:?}");
    assert!(!shown.contains("5551234") && !shown.contains("7654321"));
}

#[test]
fn local_commands_never_leave_the_laptop() {
    let (a, _b) = linked();
    for cmd in [
        CallCommand::Tones("123".into()),
        CallCommand::AudioTo(AudioSide::Laptop),
    ] {
        assert!(matches!(
            call_command(&a, &cmd),
            Err(LinkError::Protocol("local"))
        ));
    }
}

#[test]
fn oversized_history_page_is_refused() {
    let rec = CallRecord {
        id: 1,
        number: "1".into(),
        name: None,
        when_ms: 0,
        duration_s: 0,
        kind: CallKind::Incoming,
    };
    let (a, b) = linked();
    let got = thread::scope(|s| {
        s.spawn(|| {
            let (_, stream) = accept_lane(&b, &Open, T).unwrap();
            let CallIncoming::Command(_, r) = recv_calls(stream).unwrap() else {
                panic!("expected a command");
            };
            r.reply(&CallReply::HistoryPage(vec![rec; HISTORY_PAGE + 1]))
                .unwrap();
        });
        call_command(&a, &CallCommand::History { before: None })
    });
    assert!(matches!(got, Err(LinkError::Protocol("history"))));
}

fn sim(id: i32) -> Sim {
    Sim {
        id,
        slot: 0,
        name: "Airtel".into(),
        sms_default: false,
        voice_default: true,
    }
}

#[test]
fn sims_and_dial_from_one_roundtrip() {
    let sims = CallReply::Sims(vec![sim(3), sim(5)]);
    let (reply, seen) = command(&CallCommand::Sims, &sims);
    assert_eq!((reply, seen), (sims.clone(), CallCommand::Sims));
    assert!(!format!("{sims:?}").contains("Airtel"));
    let dial = CallCommand::Dial {
        number: "+1555".into(),
        sim: Some(5),
    };
    let (reply, seen) = command(&dial, &CallReply::Done);
    assert_eq!((reply, seen), (CallReply::Done, dial));
}

#[test]
fn oversized_sim_list_is_refused() {
    let (a, b) = linked();
    let got = thread::scope(|s| {
        s.spawn(|| {
            let (_, stream) = accept_lane(&b, &Open, T).unwrap();
            let CallIncoming::Command(_, r) = recv_calls(stream).unwrap() else {
                panic!("expected a command");
            };
            r.reply(&CallReply::Sims(vec![sim(1); MAX_SIMS + 1]))
                .unwrap();
        });
        call_command(&a, &CallCommand::Sims)
    });
    assert!(matches!(got, Err(LinkError::TooBig { .. })));
}

/// The dial command before SIMs, as an older build has it.
#[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
enum OldCommand {
    Dial { number: String },
}

fn cbor<T: serde::Serialize>(value: &T) -> Vec<u8> {
    let mut bytes = Vec::new();
    ciborium::into_writer(value, &mut bytes).unwrap();
    bytes
}

#[test]
fn dial_interoperates_with_older_builds() {
    let old = OldCommand::Dial {
        number: "+1555".into(),
    };
    let got: CallCommand = ciborium::from_reader(cbor(&old).as_slice()).unwrap();
    assert_eq!(
        got,
        CallCommand::Dial {
            number: "+1555".into(),
            sim: None,
        }
    );
    let new = CallCommand::Dial {
        number: "+1555".into(),
        sim: Some(2),
    };
    let got: OldCommand = ciborium::from_reader(cbor(&new).as_slice()).unwrap();
    assert_eq!(got, old);
    assert!(ciborium::from_reader::<OldCommand, _>(cbor(&CallCommand::Sims).as_slice()).is_err());
}
