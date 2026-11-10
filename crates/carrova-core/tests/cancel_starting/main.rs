//! Cancelling a call that creates a session object: a
//! [`carrova_core::Cancel`] handed to [`Importer::connect_with`] /
//! [`Importer::resume_lan_with`] stops discovery, pairing, the manifest
//! exchange and reconnecting promptly; one handed to
//! [`Exporter::start_with`] stops the scan. Either becomes the new
//! object's own canceller.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[expect(dead_code, reason = "shared test helpers; this test uses a subset")]
#[path = "../support/mod.rs"]
mod common;
#[expect(dead_code, reason = "shared test helpers; this test uses a subset")]
#[path = "../support/lan.rs"]
mod lan_support;

use std::net::{Ipv4Addr, SocketAddrV4, UdpSocket};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use carrova_core::{
    Cancel, CoreError, Event, Exporter, Importer, PlanOptions, Result, ScanOptions, State, Ticket,
    sessions,
};
use lan_support::{Side, run};

/// A cancel must end the call within this long.
const PROMPT: Duration = Duration::from_secs(2);

fn options() -> PlanOptions {
    PlanOptions {
        confirmed_sensitive: vec!["app/keys".into()],
        ..PlanOptions::default()
    }
}

/// Run `call` on a thread, cancel it after a moment and return its result
/// with how long it took to stop after the cancel.
fn cancel_soon(
    cancel: &Cancel,
    call: impl FnOnce() -> Result<Importer> + Send + 'static,
) -> (Result<Importer>, Duration) {
    let running: JoinHandle<Result<Importer>> = thread::spawn(call);
    thread::sleep(Duration::from_millis(300));
    let cancelled_at = Instant::now();
    cancel.cancel();
    let result = running.join().unwrap();
    (result, cancelled_at.elapsed())
}

/// A valid pairing code aimed at a UDP socket that never answers.
fn silent_sender(sender: &Side) -> (Ticket, UdpSocket) {
    let mut exp = sender.selected_exporter();
    let handle = exp.prepare_send().unwrap();
    let silent = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let port = silent.local_addr().unwrap().port();
    let ticket = Ticket {
        code: handle.ticket().unwrap().code.clone(),
        addr: Some(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port)),
    };
    (ticket, silent)
}

#[test]
fn cancel_while_waiting_for_a_silent_sender_stops_connecting_promptly() {
    let sender = Side::sender();
    let receiver = Side::new();
    let (ticket, _silent) = silent_sender(&sender);
    let cancel = Cancel::default();
    let engine = receiver.engine.clone();
    let flag = cancel.clone();

    let (result, took) = cancel_soon(&cancel, move || {
        Importer::connect_with(engine, &ticket, flag)
    });

    let err = result.unwrap_err();
    assert!(matches!(err, CoreError::Cancelled), "{err}");
    assert!(took < PROMPT, "stopped after {took:?}");
    assert!(sessions::list(&receiver.engine).unwrap().is_empty());
}

#[test]
fn cancel_during_the_manifest_exchange_tells_the_sender_and_leaves_no_session() {
    let sender = Side::sender();
    let receiver = Side::new();
    let mut exp = sender.selected_exporter();
    let running = run(exp.prepare_send().unwrap());
    let ticket = running.first.clone().unwrap();
    let cancel = Cancel::default();
    // Paired: the sender holds the manifest back until well after the cancel.
    let flag = cancel.clone();
    sender.sink.set_hook(move |e| {
        if matches!(e, Event::InventoryReceived(_)) {
            flag.cancel();
            thread::sleep(PROMPT + Duration::from_secs(1));
        }
    });
    let engine = receiver.engine.clone();
    let started = Instant::now();

    let err = Importer::connect_with(engine, &ticket, cancel).unwrap_err();

    assert!(matches!(err, CoreError::Cancelled), "{err}");
    assert!(started.elapsed() < PROMPT + Duration::from_secs(1));
    assert!(sessions::list(&receiver.engine).unwrap().is_empty());
    let sent = running.thread.join().unwrap().unwrap_err();
    assert!(matches!(sent, CoreError::PeerCancelled), "{sent}");
}

#[test]
fn the_given_cancel_becomes_the_importers_canceller() {
    let sender = Side::sender();
    let receiver = Side::new();
    let mut exp = sender.selected_exporter();
    let handle = exp.prepare_send().unwrap();
    let stop_sender = handle.canceller();
    let running = run(handle);
    let cancel = Cancel::default();

    let imp = Importer::connect_with(
        receiver.engine.clone(),
        running.first.as_ref().unwrap(),
        cancel.clone(),
    )
    .unwrap();

    cancel.cancel();
    assert!(imp.canceller().is_cancelled());
    stop_sender.cancel();
    let sent = running.thread.join().unwrap().unwrap_err();
    assert!(matches!(sent, CoreError::Cancelled), "{sent}");
}

/// An approved LAN import whose sender has gone away for good: resuming
/// it waits for a sender that never comes back.
fn import_without_sender(sender: &Side, receiver: &Side) -> carrova_core::SessionId {
    let mut exp = sender.selected_exporter();
    let handle = exp.prepare_send().unwrap();
    let stop_sender = handle.canceller();
    let running = run(handle);
    let mut imp =
        Importer::connect(receiver.engine.clone(), running.first.as_ref().unwrap()).unwrap();
    imp.plan(&options()).unwrap();
    imp.approve().unwrap();
    stop_sender.cancel();
    let sent = running.thread.join().unwrap().unwrap_err();
    assert!(matches!(sent, CoreError::Cancelled), "{sent}");
    imp.id().clone()
}

#[test]
fn cancel_while_resuming_stops_reconnecting_and_keeps_the_session_resumable() {
    let sender = Side::sender();
    let receiver = Side::new();
    let id = import_without_sender(&sender, &receiver);
    let cancel = Cancel::default();
    let engine = receiver.engine.clone();
    let flag = cancel.clone();
    let target = id.clone();

    let (result, took) = cancel_soon(&cancel, move || {
        Importer::resume_lan_with(engine, Some(&target), flag)
    });

    let err = result.unwrap_err();
    assert!(matches!(err, CoreError::Cancelled), "{err}");
    assert!(took < PROMPT, "stopped after {took:?}");
    let record = sessions::show(&receiver.engine, &id).unwrap();
    assert_eq!(record.state, State::Planned);
    assert!(record.approved, "approval kept for the next resume");
}

fn full_scan() -> ScanOptions {
    ScanOptions {
        include_undetected: true,
        unrecognised: true,
    }
}

#[test]
fn a_cancelled_scan_creates_no_session() {
    let sender = Side::sender();
    let cancel = Cancel::default();
    cancel.cancel();

    let err = Exporter::start_with(sender.engine.clone(), full_scan(), cancel).unwrap_err();

    assert!(matches!(err, CoreError::Cancelled), "{err}");
    assert!(sessions::list(&sender.engine).unwrap().is_empty());
}

#[test]
fn the_given_cancel_becomes_the_exporters_canceller() {
    let sender = Side::sender();
    let cancel = Cancel::default();

    let exp = Exporter::start_with(sender.engine.clone(), full_scan(), cancel.clone()).unwrap();

    cancel.cancel();
    assert!(exp.canceller().is_cancelled());
}
