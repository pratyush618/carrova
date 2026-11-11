//! Forwarded `--send` requests: argv parsing and the queue the UI drains.
#![allow(clippy::unwrap_used)]

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use carrova_core::{PickRejection, PickResult, Selection};
use tauri::async_runtime::block_on;

use super::{forward, send_paths};
use crate::ops::send;
use crate::testing::Machine;

fn argv(args: &[&str]) -> Vec<OsString> {
    args.iter().map(OsString::from).collect()
}

fn cwd() -> PathBuf {
    Path::new("/home/ana/Documents").to_path_buf()
}

fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn send_requests(machine: &Machine) -> usize {
    machine.events.recorded.of_kind("send_requested").len()
}

#[test]
fn send_paths_takes_everything_after_flag() {
    let paths = send_paths(&argv(&["carrova", "--send", "/a/b", "/c d"]), &cwd());
    assert_eq!(
        paths,
        Some(vec![PathBuf::from("/a/b"), PathBuf::from("/c d")])
    );
}

#[test]
fn relative_paths_join_cwd() {
    let paths = send_paths(&argv(&["carrova", "--send", "notes.txt"]), &cwd());
    assert_eq!(paths, Some(vec![cwd().join("notes.txt")]));
}

#[test]
fn bare_flag_is_normal_start() {
    assert_eq!(send_paths(&argv(&["carrova", "--send"]), &cwd()), None);
}

#[test]
fn no_flag_is_none() {
    assert_eq!(send_paths(&argv(&["carrova"]), &cwd()), None);
    assert_eq!(send_paths(&argv(&["carrova", "/a"]), &cwd()), None);
}

#[test]
fn burst_of_forwards_queues_every_path() {
    let machine = Machine::new();
    // Windows: one process (and so one forward) per selected file.
    for name in ["a.txt", "b.txt", "c.txt"] {
        forward(&machine.state, &[cwd().join(name)]);
    }
    let expected = ["a.txt", "b.txt", "c.txt"].map(|n| text(&cwd().join(n)));
    assert_eq!(machine.state.pending_send.take(), expected);
    assert_eq!(send_requests(&machine), 3);
    assert!(machine.state.pending_send.take().is_empty(), "taken once");
}

#[test]
fn opening_nothing_forwards_nothing() {
    let machine = Machine::new();
    forward(&machine.state, &[]);
    assert!(machine.state.pending_send.take().is_empty());
    assert_eq!(send_requests(&machine), 0);
}

/// A name Carrova cannot carry reaches the engine (lossily, as text) and
/// is refused there with a reason, instead of vanishing on the way.
#[test]
fn forwarded_non_utf8_path_is_refused_not_dropped() {
    let machine = Machine::new();
    let name = "bad\u{7}name.txt";
    machine.put(name, b"x");
    let paths = send_paths(&argv(&["carrova", "--send", name]), &machine.home).unwrap();
    forward(&machine.state, &paths);

    let queued = machine.state.pending_send.take();
    assert_eq!(queued.len(), 1);
    let opened = block_on(send::start_manual(&machine.state)).unwrap();
    let id = opened.result().session;
    let picked = queued.into_iter().map(PathBuf::from).collect();
    let reply = block_on(send::pick(
        &machine.state,
        &id,
        Selection::default(),
        picked,
    ))
    .unwrap();
    assert_eq!(
        reply.picked().outcomes[0].result,
        PickResult::Rejected {
            reason: PickRejection::UnsupportedName
        }
    );
}
