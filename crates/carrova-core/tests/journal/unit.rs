use super::*;
use carrova_platform::fake::FakePlatform;

#[test]
fn round_trip_and_torn_tail() {
    let tmp = tempfile::TempDir::new().unwrap();
    let p = FakePlatform::with_home(tmp.path());
    let path = tmp.path().join(JOURNAL_FILE);
    let mut j = Journal::create(&p, &path).unwrap();
    let a = Record::CreatedDir { abs: "/x/a".into() };
    let b = Record::Committed;
    assert_eq!(j.append(&a).unwrap(), 0);
    assert_eq!(j.append(&b).unwrap(), 1);
    drop(j);
    assert_eq!(read(&p, &path).unwrap(), vec![a.clone(), b.clone()]);

    // Torn tail: a length prefix promising more bytes than exist.
    let full = std::fs::read(&path).unwrap();
    let mut torn = full.clone();
    torn.extend_from_slice(&40u32.to_le_bytes());
    torn.extend_from_slice(b"\xa1\x01");
    std::fs::write(&path, &torn).unwrap();
    assert_eq!(read(&p, &path).unwrap(), vec![a.clone(), b.clone()]);

    // Appending truncates the torn frame first.
    let (mut j, recs) = Journal::open_append(&p, &path).unwrap();
    assert_eq!(recs.len(), 2);
    assert_eq!(j.append(&Record::RolledBack).unwrap(), 2);
    drop(j);
    assert_eq!(read(&p, &path).unwrap(), vec![a, b, Record::RolledBack]);

    // A torn 2-byte length prefix is also ignored.
    let mut bytes = std::fs::read(&path).unwrap();
    bytes.extend_from_slice(&[7, 0]);
    std::fs::write(&path, &bytes).unwrap();
    assert_eq!(read(&p, &path).unwrap().len(), 3);
}

#[test]
fn pushed_records_are_written_only_on_sync() {
    let tmp = tempfile::TempDir::new().unwrap();
    let p = FakePlatform::with_home(tmp.path());
    let path = tmp.path().join(JOURNAL_FILE);
    let mut j = Journal::create(&p, &path).unwrap();
    let a = Record::CreatedDir { abs: "/x/a".into() };
    let b = Record::CreatedTemp { abs: "/x/b".into() };
    assert_eq!(j.push(&a).unwrap(), 0);
    assert_eq!(j.push(&b).unwrap(), 1);
    assert!(read(&p, &path).unwrap().is_empty());
    j.sync().unwrap();
    assert_eq!(read(&p, &path).unwrap(), vec![a.clone(), b.clone()]);
    // `append` flushes earlier pushes with it, in order.
    j.push(&Record::Committed).unwrap();
    assert_eq!(j.append(&Record::RolledBack).unwrap(), 3);
    drop(j);
    assert_eq!(
        read(&p, &path).unwrap(),
        vec![a, b, Record::Committed, Record::RolledBack]
    );
}

#[test]
fn corrupt_complete_frame_is_error() {
    let tmp = tempfile::TempDir::new().unwrap();
    let p = FakePlatform::with_home(tmp.path());
    let path = tmp.path().join(JOURNAL_FILE);
    let mut bytes = 3u32.to_le_bytes().to_vec();
    bytes.extend_from_slice(b"\xff\xff\xff");
    std::fs::write(&path, &bytes).unwrap();
    assert!(read(&p, &path).is_err());
}
