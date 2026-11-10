use std::io::{Read, Write};
use std::path::PathBuf;

use super::*;
use crate::{Allocation, FileKind};
use tempfile::TempDir;

fn mode_of(p: &Path) -> u32 {
    crate::meta::lstat(p).unwrap().mode.unwrap() & 0o7777
}

/// `mkfifo(1)`: rustix has no `mkfifoat` on Apple targets.
fn mkfifo(path: &Path) {
    let status = std::process::Command::new("mkfifo")
        .arg(path)
        .status()
        .unwrap();
    assert!(status.success());
}

fn kind(e: &PlatformError) -> io::ErrorKind {
    match e {
        PlatformError::Io { source, .. } => source.kind(),
        other => panic!("expected Io error, got {other:?}"),
    }
}

#[test]
fn open_append_nofollow_appends_to_regular_files_only() {
    let tmp = TempDir::new().unwrap();
    let f = tmp.path().join("f");
    std::fs::write(&f, b"ab").unwrap();
    let mut h = open_append_nofollow(&f).unwrap();
    h.write_all(b"cd").unwrap();
    drop(h);
    assert_eq!(std::fs::read(&f).unwrap(), b"abcd");
    // set_len works on the handle (journal truncates a torn tail).
    let h = open_append_nofollow(&f).unwrap();
    h.set_len(1).unwrap();
    drop(h);
    assert_eq!(std::fs::read(&f).unwrap(), b"a");

    let link = tmp.path().join("l");
    std::os::unix::fs::symlink(&f, &link).unwrap();
    assert!(open_append_nofollow(&link).is_err());
    assert!(open_append_nofollow(tmp.path()).is_err());
    assert_eq!(
        kind(&open_append_nofollow(&tmp.path().join("missing")).unwrap_err()),
        io::ErrorKind::NotFound
    );
    let fifo = tmp.path().join("fifo");
    mkfifo(&fifo);
    assert!(open_append_nofollow(&fifo).is_err());
    assert_eq!(std::fs::read(&f).unwrap(), b"a");
}

#[test]
fn open_rw_nofollow_reads_and_writes_regular_files_only() {
    let tmp = TempDir::new().unwrap();
    let f = tmp.path().join("f");
    std::fs::write(&f, b"abcd").unwrap();
    let mut h = open_rw_nofollow(&f).unwrap();
    write_all_at(&h, b"XY", 1).unwrap();
    let mut s = String::new();
    h.read_to_string(&mut s).unwrap();
    assert_eq!(s, "aXYd");

    let link = tmp.path().join("l");
    std::os::unix::fs::symlink(&f, &link).unwrap();
    assert!(open_rw_nofollow(&link).is_err());
    assert!(open_rw_nofollow(tmp.path()).is_err());
    assert_eq!(
        kind(&open_rw_nofollow(&tmp.path().join("missing")).unwrap_err()),
        io::ErrorKind::NotFound
    );
    let fifo = tmp.path().join("fifo");
    mkfifo(&fifo);
    assert!(open_rw_nofollow(&fifo).is_err());
}

#[test]
fn preallocate_sizes_file_and_positional_writes_fill_it() {
    let tmp = TempDir::new().unwrap();
    let p = tmp.path().join("f");
    let f = create_new_file(&p, Some(0o600)).unwrap();
    assert_eq!(preallocate(&f, 0).unwrap(), Allocation::Reserved);
    preallocate(&f, 8).unwrap();
    assert_eq!(f.metadata().unwrap().len(), 8);
    // Out of order, like chunks arriving from parallel streams.
    write_all_at(&f, b"5678", 4).unwrap();
    write_all_at(&f, b"1234", 0).unwrap();
    drop(f);
    assert_eq!(std::fs::read(&p).unwrap(), b"12345678");
}

#[test]
fn fs_id_same_within_a_filesystem() {
    let tmp = TempDir::new().unwrap();
    std::fs::write(tmp.path().join("f"), b"x").unwrap();
    assert_eq!(
        fs_id(tmp.path()).unwrap(),
        fs_id(&tmp.path().join("f")).unwrap()
    );
    assert_eq!(
        kind(&fs_id(&tmp.path().join("missing")).unwrap_err()),
        io::ErrorKind::NotFound
    );
}

#[cfg(target_os = "linux")]
#[test]
fn sync_fs_syncs_files_and_dirs_but_not_through_links() {
    let tmp = TempDir::new().unwrap();
    let d = tmp.path().join("d");
    std::fs::create_dir(&d).unwrap();
    std::fs::write(d.join("f"), b"x").unwrap();
    sync_fs(&d).unwrap();
    sync_fs(&d.join("f")).unwrap();
    let link = tmp.path().join("l");
    std::os::unix::fs::symlink(&d, &link).unwrap();
    assert!(sync_fs(&link).is_err());
}

#[test]
fn read_dir_lists_sorted_entries_without_following_links() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().join("d");
    std::fs::create_dir(&dir).unwrap();
    std::fs::write(dir.join("b-file"), b"x").unwrap();
    std::fs::create_dir(dir.join("a-dir")).unwrap();
    std::os::unix::fs::symlink(tmp.path(), dir.join("c-link")).unwrap();
    let listed: Vec<_> = read_dir(&dir)
        .unwrap()
        .into_iter()
        .map(|e| (e.name.into_string().unwrap(), e.kind))
        .collect();
    assert_eq!(
        listed,
        [
            ("a-dir".to_owned(), FileKind::Dir),
            ("b-file".to_owned(), FileKind::File),
            ("c-link".to_owned(), FileKind::Symlink),
        ]
    );
}

#[test]
fn read_dir_refuses_symlinked_dir_and_files() {
    let tmp = TempDir::new().unwrap();
    let link = tmp.path().join("link");
    std::os::unix::fs::symlink(tmp.path(), &link).unwrap();
    assert!(read_dir(&link).is_err());
    let file = tmp.path().join("f");
    std::fs::write(&file, b"x").unwrap();
    assert!(read_dir(&file).is_err());
    assert_eq!(
        kind(&read_dir(&tmp.path().join("missing")).unwrap_err()),
        io::ErrorKind::NotFound
    );
}

#[test]
fn create_new_file_exact_modes() {
    let tmp = TempDir::new().unwrap();
    // 0o666/0o777 would be cut by any non-zero umask: proves fchmod wins.
    for mode in [0o600, 0o644, 0o666, 0o777, 0o400, 0o000] {
        let p = tmp.path().join(format!("f{mode:o}"));
        let mut f = create_new_file(&p, Some(mode)).unwrap();
        f.write_all(b"x").unwrap();
        assert_eq!(mode_of(&p), mode, "mode {mode:o}");
    }
}

#[test]
fn create_new_file_refuses_existing_file_and_symlink() {
    let tmp = TempDir::new().unwrap();
    let existing = tmp.path().join("existing");
    std::fs::write(&existing, b"keep").unwrap();
    let e = create_new_file(&existing, Some(0o600)).unwrap_err();
    assert_eq!(kind(&e), io::ErrorKind::AlreadyExists);
    assert_eq!(std::fs::read(&existing).unwrap(), b"keep");

    // Dangling symlink: must not create its target.
    let link = tmp.path().join("link");
    let target = tmp.path().join("target");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    let e = create_new_file(&link, None).unwrap_err();
    assert_eq!(kind(&e), io::ErrorKind::AlreadyExists);
    assert!(!target.exists());
}

#[test]
fn create_new_file_rejects_special_bits() {
    let tmp = TempDir::new().unwrap();
    let p = tmp.path().join("f");
    let e = create_new_file(&p, Some(0o4755)).unwrap_err();
    assert_eq!(kind(&e), io::ErrorKind::InvalidInput);
    assert!(!p.exists());
}

#[test]
fn create_dir_exact_modes_and_refuses_existing() {
    let tmp = TempDir::new().unwrap();
    for mode in [0o700, 0o755, 0o777, 0o500] {
        let p = tmp.path().join(format!("d{mode:o}"));
        create_dir(&p, Some(mode)).unwrap();
        assert_eq!(mode_of(&p), mode, "mode {mode:o}");
    }
    let e = create_dir(&tmp.path().join("d700"), Some(0o700)).unwrap_err();
    assert_eq!(kind(&e), io::ErrorKind::AlreadyExists);
    let e = create_dir(&tmp.path().join("missing/child"), None).unwrap_err();
    assert_eq!(kind(&e), io::ErrorKind::NotFound);
}

#[test]
fn open_read_nofollow_reads_files_only() {
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join("f");
    std::fs::write(&file, b"data").unwrap();
    let mut s = String::new();
    open_read_nofollow(&file)
        .unwrap()
        .read_to_string(&mut s)
        .unwrap();
    assert_eq!(s, "data");

    let link = tmp.path().join("link");
    std::os::unix::fs::symlink(&file, &link).unwrap();
    assert!(open_read_nofollow(&link).is_err());

    let e = open_read_nofollow(tmp.path()).unwrap_err();
    assert_eq!(kind(&e), io::ErrorKind::InvalidInput);

    let fifo = tmp.path().join("fifo");
    mkfifo(&fifo);
    let e = open_read_nofollow(&fifo).unwrap_err();
    assert_eq!(kind(&e), io::ErrorKind::InvalidInput);
}

#[test]
fn set_mode_exact_and_refuses_symlink() {
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join("f");
    std::fs::write(&file, b"x").unwrap();
    set_mode(&file, 0o000).unwrap();
    assert_eq!(mode_of(&file), 0o000);
    // Works without read permission on the object.
    set_mode(&file, 0o640).unwrap();
    assert_eq!(mode_of(&file), 0o640);

    let dir = tmp.path().join("d");
    std::fs::create_dir(&dir).unwrap();
    set_mode(&dir, 0o711).unwrap();
    assert_eq!(mode_of(&dir), 0o711);

    let link = tmp.path().join("link");
    std::os::unix::fs::symlink(&file, &link).unwrap();
    assert!(set_mode(&link, 0o777).is_err());
    assert_eq!(mode_of(&file), 0o640, "symlink target untouched");

    assert!(set_mode(&file, 0o1777).is_err());
    assert_eq!(mode_of(&file), 0o640);
}

#[test]
fn symlink_roundtrip_is_verbatim() {
    let tmp = TempDir::new().unwrap();
    let link = tmp.path().join("l");
    create_symlink("../not/resolved", &link).unwrap();
    assert_eq!(read_link(&link).unwrap(), PathBuf::from("../not/resolved"));
    let e = create_symlink("x", &link).unwrap_err();
    assert_eq!(kind(&e), io::ErrorKind::AlreadyExists);
    assert!(read_link(&tmp.path().join("missing")).is_err());
}

#[test]
fn rename_noreplace_semantics() {
    let tmp = TempDir::new().unwrap();
    let a = tmp.path().join("a");
    let b = tmp.path().join("b");
    std::fs::write(&a, b"A").unwrap();
    std::fs::write(&b, b"B").unwrap();
    let e = rename_noreplace(&a, &b).unwrap_err();
    assert_eq!(kind(&e), io::ErrorKind::AlreadyExists);
    assert_eq!(std::fs::read(&a).unwrap(), b"A");
    assert_eq!(std::fs::read(&b).unwrap(), b"B");

    // Dangling symlink at the destination also counts as existing.
    let l = tmp.path().join("l");
    std::os::unix::fs::symlink(tmp.path().join("nowhere"), &l).unwrap();
    assert!(rename_noreplace(&a, &l).is_err());

    let c = tmp.path().join("c");
    rename_noreplace(&a, &c).unwrap();
    assert!(!a.exists());
    assert_eq!(std::fs::read(&c).unwrap(), b"A");
}

#[test]
fn rename_replace_swaps_the_destination_and_never_follows_it() {
    let tmp = TempDir::new().unwrap();
    let (a, b, target) = (
        tmp.path().join("a"),
        tmp.path().join("b"),
        tmp.path().join("target"),
    );
    std::fs::write(&a, b"A").unwrap();
    std::fs::write(&b, b"B").unwrap();
    rename_replace(&a, &b).unwrap();
    assert!(!a.exists());
    assert_eq!(std::fs::read(&b).unwrap(), b"A");

    // A symlink at the destination is replaced, its target untouched.
    std::fs::write(&target, b"T").unwrap();
    let link = tmp.path().join("link");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    rename_replace(&b, &link).unwrap();
    assert_eq!(std::fs::read(&target).unwrap(), b"T");
    assert!(!std::fs::symlink_metadata(&link).unwrap().is_symlink());
    assert_eq!(std::fs::read(&link).unwrap(), b"A");
}

#[test]
fn sync_dir_and_remove_file() {
    let tmp = TempDir::new().unwrap();
    sync_dir(tmp.path()).unwrap();
    let f = tmp.path().join("f");
    std::fs::write(&f, b"x").unwrap();
    assert!(sync_dir(&f).is_err());

    // remove_file on a symlink removes the link, not its target.
    let link = tmp.path().join("link");
    std::os::unix::fs::symlink(&f, &link).unwrap();
    assert!(sync_dir(&link).is_err());
    remove_file(&link).unwrap();
    assert!(f.exists());
    remove_file(&f).unwrap();
    assert!(!f.exists());
    assert!(remove_file(tmp.path()).is_err());
}

#[test]
fn remove_dir_all_confined_refuses_outside_and_equal() {
    let tmp = TempDir::new().unwrap();
    let within = tmp.path().join("state");
    let other = tmp.path().join("other");
    std::fs::create_dir_all(within.join("sub")).unwrap();
    std::fs::create_dir_all(&other).unwrap();

    for bad in [
        within.clone(),
        within.join("."),
        within.join("sub/.."),
        other.clone(),
        within.join("../other"),
        tmp.path().to_path_buf(),
        PathBuf::from("relative"),
    ] {
        let e = remove_dir_all_confined(&bad, &within).unwrap_err();
        assert_eq!(kind(&e), io::ErrorKind::InvalidInput, "{}", bad.display());
    }
    assert!(within.join("sub").is_dir());
    assert!(other.is_dir());

    let e = remove_dir_all_confined(&within.join("missing"), &within).unwrap_err();
    assert_eq!(kind(&e), io::ErrorKind::NotFound);
}

#[test]
fn remove_dir_all_confined_never_follows_symlinks() {
    let tmp = TempDir::new().unwrap();
    let outside = tmp.path().join("outside");
    std::fs::create_dir_all(outside.join("deep")).unwrap();
    std::fs::write(outside.join("deep/precious"), b"keep").unwrap();

    let within = tmp.path().join("state");
    let victim = within.join("session");
    std::fs::create_dir_all(victim.join("a/b")).unwrap();
    std::fs::write(victim.join("a/b/file"), b"x").unwrap();
    std::os::unix::fs::symlink(&outside, victim.join("a/link_dir")).unwrap();
    std::os::unix::fs::symlink(outside.join("deep/precious"), victim.join("link_file")).unwrap();

    remove_dir_all_confined(&victim, &within).unwrap();
    assert!(!victim.exists());
    assert!(within.is_dir());
    assert_eq!(
        std::fs::read(outside.join("deep/precious")).unwrap(),
        b"keep"
    );

    // A symlink *as* the target is unlinked, not traversed.
    let link = within.join("link");
    std::os::unix::fs::symlink(&outside, &link).unwrap();
    remove_dir_all_confined(&link, &within).unwrap();
    assert!(std::fs::symlink_metadata(&link).is_err());
    assert!(outside.join("deep/precious").exists());

    // A symlink *ancestor* below `within` aborts the walk.
    let hop = within.join("hop");
    std::os::unix::fs::symlink(&outside, &hop).unwrap();
    assert!(remove_dir_all_confined(&hop.join("deep"), &within).is_err());
    assert!(outside.join("deep/precious").exists());
}
