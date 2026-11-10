//! "Show a new code": a LAN send that ended before any receiver paired
//! returns to its selection, and the same exporter sends again with a
//! fresh code.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[expect(dead_code, reason = "shared test helpers; this test uses a subset")]
#[path = "../support/mod.rs"]
mod common;
#[expect(dead_code, reason = "shared test helpers; this test uses a subset")]
#[path = "../support/lan.rs"]
mod lan_support;

use carrova_core::{CoreError, Importer, PlanOptions, SendOutcome, State, Ticket, sessions};
use carrova_crypto::pairing::PairingCode;
use lan_support::{Side, run, run_answering};

fn options() -> PlanOptions {
    PlanOptions {
        confirmed_sensitive: vec!["app/keys".into()],
        ..PlanOptions::default()
    }
}

/// A different code on the same nameplate.
fn other_words(code: &PairingCode) -> PairingCode {
    loop {
        let fresh = PairingCode::generate().unwrap().to_string();
        let words = fresh.split_once('-').unwrap().1;
        let other: PairingCode = format!("{}-{words}", code.nameplate()).parse().unwrap();
        if &other != code {
            return other;
        }
    }
}

/// A receiver pairs with `ticket` and applies everything.
fn receive(receiver: &Side, ticket: &Ticket) {
    let mut imp = Importer::connect(receiver.engine.clone(), ticket).unwrap();
    imp.plan(&options()).unwrap();
    imp.approve().unwrap();
    imp.apply().unwrap();
}

#[test]
fn a_send_cancelled_before_pairing_returns_to_the_selection() {
    let sender = Side::sender();
    let mut exp = sender.selected_exporter();
    let handle = exp.prepare_send().unwrap();
    handle.canceller().cancel();

    let (result, _, _) = run(handle).finish();

    assert!(matches!(result, Err(CoreError::Cancelled)));
    let record = sessions::show(&sender.engine, exp.id()).unwrap();
    assert_eq!(record.state, State::Selected);
    assert_eq!(record.source, None, "no send is pending");
}

#[test]
fn after_a_cancel_the_same_exporter_sends_again_with_a_fresh_code() {
    let sender = Side::sender();
    let receiver = Side::new();
    let mut exp = sender.selected_exporter();
    let first = exp.prepare_send().unwrap();
    let old_code = first.ticket().unwrap().code.clone();
    first.canceller().cancel();
    let _ = run(first).finish();

    assert!(exp.reopen_after_send().unwrap());
    let again = exp.prepare_send().unwrap();
    let running = run(again);
    let ticket = running.first.clone().unwrap();
    assert_ne!(ticket.code, old_code, "a fresh code");
    receive(&receiver, &ticket);

    let (result, _, _) = running.finish();
    assert_eq!(result.unwrap().outcome, SendOutcome::Applied);
    assert_eq!(
        sessions::show(&sender.engine, exp.id()).unwrap().state,
        State::Done
    );
}

#[test]
fn declining_a_new_code_returns_to_the_selection_too() {
    let sender = Side::sender();
    let guesser = Side::new();
    let mut exp = sender.selected_exporter();
    let running = run_answering(exp.prepare_send().unwrap(), false);
    let first = running.first.clone().unwrap();
    let wrong = Ticket {
        code: other_words(&first.code),
        addr: first.addr,
    };
    assert!(Importer::connect(guesser.engine.clone(), &wrong).is_err());

    let (result, _, _) = running.finish();

    assert!(matches!(result, Err(CoreError::PairingStopped { .. })));
    assert!(exp.reopen_after_send().unwrap());
}

#[test]
fn a_send_that_ran_to_the_end_cannot_be_reopened() {
    let sender = Side::sender();
    let receiver = Side::new();
    let mut exp = sender.selected_exporter();
    let running = run(exp.prepare_send().unwrap());
    receive(&receiver, running.first.as_ref().unwrap());
    running.finish().0.unwrap();

    assert!(!exp.reopen_after_send().unwrap());
    assert!(exp.prepare_send().is_err());
}
