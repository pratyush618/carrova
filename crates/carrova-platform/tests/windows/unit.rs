use std::ffi::OsStr;
use std::io;
use std::path::Path;

use super::fs::*;
use super::roots::confine;
use super::*;
use tempfile::TempDir;

fn kind(e: &PlatformError) -> io::ErrorKind {
    match e {
        PlatformError::Io { source, .. } => source.kind(),
        other => panic!("expected Io error, got {other:?}"),
    }
}

#[test]
fn roots_refuse_drive_and_share_roots() {
    for bad in [r"C:\", r"C:\Users\..", r"\\server\share\", r"C:relative"] {
        assert!(
            confine(SemanticRoot::Home, Path::new(bad)).is_err(),
            "{bad}"
        );
    }
    let ok = confine(SemanticRoot::Home, Path::new(r"C:\Users\me\.\x\..")).unwrap();
    assert_eq!(ok, Path::new(r"C:\Users\me"));
}

#[test]
fn real_roots_and_state_dir_are_confined() {
    let p = WindowsPlatform;
    for root in [
        SemanticRoot::UserConfig,
        SemanticRoot::UserData,
        SemanticRoot::Cache,
        SemanticRoot::Home,
        SemanticRoot::Documents,
    ] {
        assert!(p.resolve_root(root).unwrap().parent().is_some(), "{root:?}");
    }
    assert!(p.state_dir().unwrap().ends_with(r"Carrova\state"));
}

#[test]
fn rename_noreplace_refuses_existing_target() {
    let tmp = TempDir::new().unwrap();
    let (a, b) = (tmp.path().join("a"), tmp.path().join("b"));
    std::fs::write(&a, b"a").unwrap();
    std::fs::write(&b, b"b").unwrap();
    let err = rename_noreplace(&a, &b).unwrap_err();
    assert_eq!(kind(&err), io::ErrorKind::AlreadyExists);
    assert_eq!(std::fs::read(&a).unwrap(), b"a");

    let (d, e) = (tmp.path().join("d"), tmp.path().join("e"));
    std::fs::create_dir(&d).unwrap();
    std::fs::create_dir(&e).unwrap();
    assert!(rename_noreplace(&d, &e).is_err());
    assert_eq!(
        kind(&rename_noreplace(&d, &b).unwrap_err()),
        io::ErrorKind::AlreadyExists
    );
    assert_eq!(std::fs::read(&b).unwrap(), b"b");

    let c = tmp.path().join("c");
    rename_noreplace(&a, &c).unwrap();
    assert!(!a.exists());
    assert_eq!(std::fs::read(&c).unwrap(), b"a");
}

/// Symlinks need Developer Mode or admin; skip when not creatable.
fn try_symlink(target: &Path, link: &Path) -> bool {
    match create_symlink(&target.to_string_lossy(), link) {
        Ok(()) => true,
        Err(e) if kind(&e) == io::ErrorKind::PermissionDenied => false,
        Err(e) => panic!("{e}"),
    }
}

#[test]
fn nofollow_opens_reject_symlinks() {
    let tmp = TempDir::new().unwrap();
    let f = tmp.path().join("f");
    std::fs::write(&f, b"x").unwrap();
    let link = tmp.path().join("l");
    if !try_symlink(&f, &link) {
        return;
    }
    assert!(open_read_nofollow(&link).is_err());
    assert!(open_rw_nofollow(&link).is_err());
    assert!(open_append_nofollow(&link).is_err());
    assert!(lock_exclusive(&link).is_err());
    assert!(set_mode(&link, 0o600).is_err());
    assert!(create_new_file(&link, None).is_err());
    assert_eq!(read_link(&link).unwrap(), f);
    remove_file(&link).unwrap();
    assert_eq!(std::fs::read(&f).unwrap(), b"x");
}

#[test]
fn opens_accept_files_only_and_append_after_set_len() {
    use std::io::{Seek, SeekFrom, Write};
    let tmp = TempDir::new().unwrap();
    let f = tmp.path().join("f");
    std::fs::write(&f, b"abcd").unwrap();
    assert!(open_read_nofollow(tmp.path()).is_err());
    set_mode(&f, 0o600).unwrap();
    set_mode(tmp.path(), 0o700).unwrap();

    let mut h = open_append_nofollow(&f).unwrap();
    h.set_len(1).unwrap();
    h.seek(SeekFrom::End(0)).unwrap();
    h.write_all(b"z").unwrap();
    drop(h);
    assert_eq!(std::fs::read(&f).unwrap(), b"az");
}

#[test]
fn positional_writes_and_dir_listing() {
    let tmp = TempDir::new().unwrap();
    let f = tmp.path().join("b");
    let h = create_new_file(&f, Some(0o600)).unwrap();
    assert_eq!(preallocate(&h, 4).unwrap(), Allocation::Sparse);
    write_all_at(&h, b"XY", 1).unwrap();
    drop(h);
    assert_eq!(std::fs::read(&f).unwrap(), b"\0XY\0");
    create_dir(&tmp.path().join("a"), None).unwrap();
    let names: Vec<_> = read_dir(tmp.path()).unwrap();
    assert_eq!(names[0].name, "a");
    assert_eq!(names[0].kind, crate::FileKind::Dir);
    assert_eq!(names[1].kind, crate::FileKind::File);
    assert_eq!(fs_id(&f).unwrap(), fs_id(tmp.path()).unwrap());
}

#[test]
fn remove_dir_all_confined_stays_inside() {
    let tmp = TempDir::new().unwrap();
    let tree = tmp.path().join("t");
    std::fs::create_dir_all(tree.join("x")).unwrap();
    std::fs::write(tree.join("x").join("f"), b"x").unwrap();
    assert!(remove_dir_all_confined(tmp.path(), tmp.path()).is_err());
    assert!(remove_dir_all_confined(&tree, &tree.join("x")).is_err());
    remove_dir_all_confined(&tree, tmp.path()).unwrap();
    assert!(!tree.exists());
}

#[test]
fn lock_excludes_a_second_locker() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("lock");
    let _held = lock_exclusive(&path).unwrap();
    let other = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
    assert!(matches!(
        other.try_lock(),
        Err(std::fs::TryLockError::WouldBlock)
    ));
}

#[test]
fn find_binary_uses_pathext_and_refuses_paths() {
    let tmp = TempDir::new().unwrap();
    std::fs::write(tmp.path().join("tool.CMD"), b"").unwrap();
    let path_var = std::env::join_paths([tmp.path(), Path::new("relative")]).unwrap();
    let exts = ".EXE;.CMD";
    assert_eq!(
        search_path(&path_var, exts, "tool"),
        Some(tmp.path().join("tool.CMD"))
    );
    assert_eq!(
        search_path(&path_var, exts, "tool.CMD"),
        Some(tmp.path().join("tool.CMD"))
    );
    for bad in ["", ".", "..", r"a\tool", "a/tool", "C:tool", "missing"] {
        assert_eq!(search_path(&path_var, exts, bad), None, "{bad}");
    }
}

#[test]
fn process_names_are_exe_stems() {
    assert_eq!(process_name(OsStr::new("Code.exe")), "Code");
    assert_eq!(process_name(OsStr::new("gpg-agent")), "gpg-agent");
}

#[test]
fn disk_is_the_volume_holding_a_path() {
    let tmp = TempDir::new().unwrap();
    let p = WindowsPlatform;
    let disk = p.disk(tmp.path()).unwrap();
    assert!(disk.total >= p.free_space(tmp.path()).unwrap());
    assert!(tmp.path().starts_with(&disk.mount_point), "{disk:?}");
}
