#![allow(clippy::unwrap_used)]

use std::thread;
use std::time::Instant;

use carrova_core::Cancel;
use carrova_view::ecosystem::PairPeerView;

use super::*;
use crate::registry::Registry;
use crate::testing::Recorder;

fn session() -> SessionId {
    SessionId("20261108-101500-a1b2c3".into())
}

fn prompt(
    recorder: &Arc<Recorder>,
    answers: &Arc<PairingAnswers>,
    timeout: Duration,
) -> BridgePrompt {
    BridgePrompt::new(session(), recorder.clone(), answers.clone(), timeout)
}

/// Answer `yes` as soon as the question is shown.
fn answer_when_asked(recorder: Arc<Recorder>, answers: Arc<PairingAnswers>, yes: bool) {
    thread::spawn(move || {
        while recorder.of_kind("confirm_new_code").is_empty() {
            thread::sleep(Duration::from_millis(5));
        }
        answers.answer(&session(), yes).unwrap();
    });
}

#[test]
fn codes_are_emitted_for_display() {
    let recorder = Arc::new(Recorder::default());
    let answers = Arc::new(PairingAnswers::default());
    let ticket: Ticket = "7-acid-lemon".parse().unwrap();
    prompt(&recorder, &answers, ANSWER_TIMEOUT).show_code(
        &ticket,
        CodeOffer::Renewed,
        Duration::from_secs(300),
    );
    let shown = recorder.of_kind("pairing_code");
    assert_eq!(shown.len(), 1);
    assert_eq!(shown[0]["code"]["code"], "7-acid-lemon");
    assert_eq!(shown[0]["code"]["renewed"], true);
    assert_eq!(shown[0]["code"]["expires_in_secs"], 300);
}

#[test]
fn the_users_answer_decides_whether_a_new_code_is_offered() {
    for yes in [true, false] {
        let recorder = Arc::new(Recorder::default());
        let answers = Arc::new(PairingAnswers::default());
        answer_when_asked(recorder.clone(), answers.clone(), yes);
        let mut prompt = prompt(&recorder, &answers, ANSWER_TIMEOUT);
        assert_eq!(prompt.confirm_new_code(1, 2), yes);
        let asked = recorder.of_kind("confirm_new_code");
        assert_eq!(asked[0]["failures"], 1);
        assert_eq!(asked[0]["remaining"], 2);
    }
}

#[test]
fn no_answer_in_time_means_no_new_code() {
    let recorder = Arc::new(Recorder::default());
    let answers = Arc::new(PairingAnswers::default());
    let started = Instant::now();
    let mut prompt = prompt(&recorder, &answers, Duration::from_millis(50));
    assert!(!prompt.confirm_new_code(1, 2));
    assert!(started.elapsed() < Duration::from_secs(5));
    assert_eq!(
        answers.answer(&session(), true),
        Err(NoQuestion),
        "question closed"
    );
}

#[test]
fn answering_without_a_question_fails() {
    let answers = PairingAnswers::default();
    assert_eq!(answers.answer(&session(), true), Err(NoQuestion));
}

fn found() -> PairPeerView {
    PairPeerView {
        name: "Pixel 8".into(),
        os: carrova_platform::OsKind::Android,
        fingerprint: "7F3A·19C2".into(),
    }
}

/// Ask `found()` on a thread, within `window`; returns the answer's handle.
fn ask_on_thread(
    recorder: &Arc<Recorder>,
    peers: &Arc<PeerAnswer>,
    cancel: &Cancel,
    window: Duration,
) -> thread::JoinHandle<bool> {
    let (out, peers, cancel) = (recorder.clone(), peers.clone(), cancel.clone());
    let asked = thread::spawn(move || peers.ask(&*out, found(), &cancel, window));
    while recorder.of_kind("pair_peer_found").is_empty() {
        thread::sleep(Duration::from_millis(5));
    }
    asked
}

#[test]
fn the_users_answer_decides_whether_the_peer_is_linked() {
    for yes in [true, false] {
        let recorder = Arc::new(Recorder::default());
        let peers = Arc::new(PeerAnswer::default());
        let asked = ask_on_thread(&recorder, &peers, &Cancel::default(), ANSWER_TIMEOUT);
        peers.answer(yes).unwrap();
        assert_eq!(asked.join().unwrap(), yes);
        let shown = recorder.of_kind("pair_peer_found");
        assert_eq!(shown[0]["peer"]["fingerprint"], "7F3A·19C2");
    }
}

#[test]
fn an_unanswered_peer_question_is_a_no_and_closes() {
    let recorder = Arc::new(Recorder::default());
    let peers = Arc::new(PeerAnswer::default());
    let asked = ask_on_thread(
        &recorder,
        &peers,
        &Cancel::default(),
        Duration::from_millis(50),
    );
    assert!(!asked.join().unwrap());
    assert_eq!(peers.answer(true), Err(NoQuestion), "question closed");
}

#[test]
fn cancel_connect_unblocks_peer_question() {
    let recorder = Arc::new(Recorder::default());
    let peers = Arc::new(PeerAnswer::default());
    let registry = Registry::default();
    let cancel = registry.begin_connect().unwrap();
    let asked = ask_on_thread(&recorder, &peers, &cancel, ANSWER_TIMEOUT);

    let started = Instant::now();
    registry.cancel_connect();

    assert!(!asked.join().unwrap());
    assert!(started.elapsed() < Duration::from_secs(2));
}
