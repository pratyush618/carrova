use clap::Parser;

use super::{ReceiveArgs, SendArgs};
use crate::output::OutputFormat;
use crate::prompt::Consent;

#[derive(Debug, Parser)]
struct Send {
    #[command(flatten)]
    args: SendArgs,
}

#[derive(Debug, Parser)]
struct Receive {
    #[command(flatten)]
    args: ReceiveArgs,
}

fn send(argv: &[&str]) -> Result<SendArgs, clap::Error> {
    Send::try_parse_from(std::iter::once("send").chain(argv.iter().copied())).map(|s| s.args)
}

fn receive(argv: &[&str]) -> Result<ReceiveArgs, clap::Error> {
    Receive::try_parse_from(std::iter::once("receive").chain(argv.iter().copied())).map(|r| r.args)
}

#[test]
fn send_takes_export_selection_flags() {
    let a = send(&[
        "--include",
        "ssh/keys",
        "--confirm-sensitive",
        "ssh/keys",
        "-y",
    ])
    .unwrap();
    assert!(a.yes);
    assert!(a.resume.is_none());
}

#[test]
fn send_resume_with_and_without_session() {
    assert_eq!(send(&["--resume"]).unwrap().resume, Some(None));
    let a = send(&["--resume", "20261108-101500-a1b2c3"]).unwrap();
    assert_eq!(
        a.resume.unwrap().map(|s| s.0),
        Some("20261108-101500-a1b2c3".to_owned())
    );
    assert!(send(&["--resume", "../x"]).is_err());
}

#[test]
fn send_resume_rejects_new_selection() {
    let e = send(&["--resume", "--include", "git/config"]).unwrap_err();
    assert_eq!(e.kind(), clap::error::ErrorKind::ArgumentConflict);
}

#[test]
fn receive_takes_code_or_ticket() {
    let a = receive(&["7-acid-lemon"]).unwrap();
    assert_eq!(a.ticket.unwrap().addr, None);
    let a = receive(&["7-acid-lemon@10.0.0.2:47822", "--dry-run"]).unwrap();
    assert!(a.ticket.unwrap().addr.is_some());
    assert!(receive(&[]).unwrap().ticket.is_none());
}

#[test]
fn receive_rejects_bad_code_without_echoing_it() {
    let e = receive(&["7-acid-lemonade"]).unwrap_err().to_string();
    assert!(e.contains("not a pairing code"), "{e}");
    assert!(!e.contains("lemon"), "{e}");
}

#[test]
fn receive_resume_conflicts_with_code() {
    let e = receive(&["7-acid-lemon", "--resume"]).unwrap_err();
    assert_eq!(e.kind(), clap::error::ErrorKind::ArgumentConflict);
    assert_eq!(receive(&["--resume"]).unwrap().resume, Some(None));
}

#[test]
fn receive_takes_import_plan_flags() {
    let a = receive(&[
        "7-acid-lemon",
        "--rule",
        "default=skip",
        "--remap",
        "git/config=user_data",
        "--keep-group-write",
        "--import-trust",
        "--yes",
        "--json",
    ])
    .unwrap();
    assert!(a.plan.consent() == Consent::Assume && a.plan.format() == OutputFormat::Json);
    assert!(receive(&["7-acid-lemon", "--dry-run", "--yes"]).is_err());
}
