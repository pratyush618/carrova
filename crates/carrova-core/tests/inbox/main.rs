//! The notification inbox: what the bell lists and keeps.
#![allow(clippy::unwrap_used)]

use std::path::PathBuf;
use std::sync::Arc;

use carrova_core::time::SECS_PER_DAY;
use carrova_core::{Inbox, NoticeKind, RETENTION_DAYS};
use carrova_platform::Platform;
use carrova_platform::fake::FakePlatform;
use tempfile::TempDir;

const NOW: u64 = 1_800_000_000;

fn platform(home: &TempDir) -> Arc<dyn Platform> {
    Arc::new(FakePlatform::with_home(home.path()))
}

fn inbox_file(home: &TempDir) -> PathBuf {
    home.path().join(".local/state/carrova/inbox.json")
}

#[test]
fn inbox_lists_newest_first() {
    let home = TempDir::new().unwrap();
    let inbox = Inbox::open_at(platform(&home), NOW).unwrap();
    inbox
        .push(NoticeKind::MoveFinished, NOW - 20, None, "Done".into())
        .unwrap();
    inbox
        .push(
            NoticeKind::SendRequest,
            NOW - 10,
            Some("Ana".into()),
            "Wants".into(),
        )
        .unwrap();
    let kinds: Vec<_> = inbox.list().iter().map(|n| n.kind).collect();
    assert_eq!(kinds, [NoticeKind::SendRequest, NoticeKind::MoveFinished]);
}

#[test]
fn inbox_drops_entries_older_than_30_days() {
    let home = TempDir::new().unwrap();
    let old = NOW - RETENTION_DAYS * SECS_PER_DAY - 1;
    let first = Inbox::open_at(platform(&home), old).unwrap();
    first
        .push(NoticeKind::UpdateReady, old, None, "old".into())
        .unwrap();
    first
        .push(
            NoticeKind::DeviceNearby,
            NOW - SECS_PER_DAY,
            None,
            "new".into(),
        )
        .unwrap();

    let reopened = Inbox::open_at(platform(&home), NOW).unwrap();
    let summaries: Vec<_> = reopened.list().into_iter().map(|n| n.summary).collect();
    assert_eq!(summaries, ["new"]);
}

#[test]
fn inbox_file_is_0600() {
    let home = TempDir::new().unwrap();
    let inbox = Inbox::open_at(platform(&home), NOW).unwrap();
    inbox
        .push(NoticeKind::MoveFailed, NOW, None, "x".into())
        .unwrap();
    let mode = platform(&home).metadata(&inbox_file(&home)).unwrap().mode;
    assert_eq!(mode.map(|m| m & 0o777), Some(0o600));
}

#[test]
fn read_marks_survive_a_restart() {
    let home = TempDir::new().unwrap();
    let inbox = Inbox::open_at(platform(&home), NOW).unwrap();
    let a = inbox
        .push(NoticeKind::MoveFinished, NOW, None, "a".into())
        .unwrap();
    inbox
        .push(NoticeKind::MoveFinished, NOW, None, "b".into())
        .unwrap();
    inbox.mark_read(&[a.id]).unwrap();
    let unread = |i: &Inbox| i.list().iter().filter(|n| !n.read).count();
    assert_eq!(unread(&Inbox::open_at(platform(&home), NOW).unwrap()), 1);
    inbox.mark_all_read().unwrap();
    assert_eq!(unread(&Inbox::open_at(platform(&home), NOW).unwrap()), 0);
}
