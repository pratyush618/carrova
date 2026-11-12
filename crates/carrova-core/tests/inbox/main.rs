//! The notification inbox: what the bell lists and keeps.
#![allow(clippy::unwrap_used)]

use std::path::PathBuf;
use std::sync::Arc;

use carrova_core::time::SECS_PER_DAY;
use carrova_core::{Inbox, NoticeKind, RETENTION_DAYS};
use carrova_platform::fake::FakePlatform;
use carrova_platform::{PermissionModel, Platform};
use tempfile::TempDir;

const NOW: u64 = 1_800_000_000;

fn platform(home: &TempDir) -> Arc<dyn Platform> {
    Arc::new(FakePlatform::with_home(home.path()))
}

fn open(home: &TempDir, now: u64) -> Inbox {
    let (inbox, set_aside) = Inbox::open_at(platform(home), now).unwrap();
    assert_eq!(set_aside, None);
    inbox
}

fn inbox_file(home: &TempDir) -> PathBuf {
    home.path().join(".local/state/carrova/inbox.json")
}

#[test]
fn inbox_lists_newest_first() {
    let home = TempDir::new().unwrap();
    let inbox = open(&home, NOW);
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
    let first = open(&home, old);
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

    let reopened = open(&home, NOW);
    let summaries: Vec<_> = reopened.list().into_iter().map(|n| n.summary).collect();
    assert_eq!(summaries, ["new"]);
}

#[test]
fn inbox_drops_old_entries_while_running() {
    let home = TempDir::new().unwrap();
    let old = NOW - RETENTION_DAYS * SECS_PER_DAY - 1;
    let inbox = open(&home, old);
    inbox
        .push(NoticeKind::UpdateReady, old, None, "old".into())
        .unwrap();
    inbox
        .push(NoticeKind::DeviceNearby, NOW, None, "new".into())
        .unwrap();

    let summaries: Vec<_> = inbox.list().into_iter().map(|n| n.summary).collect();
    assert_eq!(summaries, ["new"]);
}

#[test]
fn inbox_file_is_0600() {
    let home = TempDir::new().unwrap();
    let inbox = open(&home, NOW);
    inbox
        .push(NoticeKind::MoveFailed, NOW, None, "x".into())
        .unwrap();
    let platform = platform(&home);
    let mode = platform.metadata(&inbox_file(&home)).unwrap().mode;
    // Only Unix has modes; elsewhere the file carries none to check.
    if platform.permission_model() == PermissionModel::Unix {
        assert_eq!(mode.map(|m| m & 0o777), Some(0o600));
    }
}

#[test]
fn read_marks_survive_a_restart() {
    let home = TempDir::new().unwrap();
    let inbox = open(&home, NOW);
    let a = inbox
        .push(NoticeKind::MoveFinished, NOW, None, "a".into())
        .unwrap();
    inbox
        .push(NoticeKind::MoveFinished, NOW, None, "b".into())
        .unwrap();
    inbox.mark_read(&[a.id]).unwrap();
    let unread = |i: &Inbox| i.list().iter().filter(|n| !n.read).count();
    assert_eq!(unread(&open(&home, NOW)), 1);
    inbox.mark_all_read().unwrap();
    assert_eq!(unread(&open(&home, NOW)), 0);
}

#[test]
fn a_corrupt_inbox_is_set_aside_not_fatal() {
    let home = TempDir::new().unwrap();
    let file = inbox_file(&home);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, b"{ not json").unwrap();

    let (inbox, set_aside) = Inbox::open_at(platform(&home), NOW).unwrap();
    assert!(inbox.list().is_empty());
    let aside = set_aside.unwrap();
    assert_eq!(
        aside,
        file.with_file_name(format!("inbox.json.corrupt-{NOW}"))
    );
    assert_eq!(std::fs::read(&aside).unwrap(), b"{ not json");
    assert!(!file.exists());
    inbox
        .push(NoticeKind::MoveFinished, NOW, None, "ok".into())
        .unwrap();
    assert_eq!(open(&home, NOW).list().len(), 1);
}

#[test]
fn a_read_mark_that_cannot_be_saved_is_not_kept() {
    let home = TempDir::new().unwrap();
    let inbox = open(&home, NOW);
    inbox
        .push(NoticeKind::MoveFinished, NOW, None, "a".into())
        .unwrap();
    // A file where the state directory was: nothing can be written.
    let state = inbox_file(&home).parent().unwrap().to_owned();
    std::fs::remove_dir_all(&state).unwrap();
    std::fs::write(&state, b"").unwrap();

    assert!(inbox.mark_all_read().is_err());
    assert!(inbox.list().iter().all(|n| !n.read));
}

#[test]
fn a_notice_never_takes_the_id_of_an_expired_one() {
    let home = TempDir::new().unwrap();
    let old = NOW - RETENTION_DAYS * SECS_PER_DAY - 1;
    let inbox = open(&home, old);
    let expired = inbox
        .push(NoticeKind::UpdateReady, old, None, "old".into())
        .unwrap();

    let fresh = inbox
        .push(NoticeKind::MoveFinished, NOW, None, "new".into())
        .unwrap();

    assert_eq!(inbox.list().len(), 1, "the old one expired");
    assert_ne!(fresh.id, expired.id);
}
