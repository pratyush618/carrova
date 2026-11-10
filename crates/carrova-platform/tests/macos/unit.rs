use std::io;
use std::sync::mpsc;
use std::time::Duration;

use super::*;
use tempfile::TempDir;

fn env_home(home: &str) -> MacEnv {
    MacEnv {
        home: Some(home.into()),
        ..MacEnv::default()
    }
}

fn root(p: &MacPlatform, r: SemanticRoot) -> PathBuf {
    p.resolve_root(r).unwrap()
}

fn io_kind(e: &PlatformError) -> io::ErrorKind {
    match e {
        PlatformError::Io { source, .. } => source.kind(),
        other => panic!("expected Io error, got {other:?}"),
    }
}

// ---- roots ----

#[test]
fn roots_follow_the_library_layout() {
    let p = MacPlatform::new(env_home("/Users/u"));
    let support = PathBuf::from("/Users/u/Library/Application Support");
    assert_eq!(root(&p, SemanticRoot::Home), PathBuf::from("/Users/u"));
    assert_eq!(root(&p, SemanticRoot::UserConfig), support);
    assert_eq!(root(&p, SemanticRoot::UserData), support);
    assert_eq!(
        root(&p, SemanticRoot::Cache),
        PathBuf::from("/Users/u/Library/Caches")
    );
    assert_eq!(
        root(&p, SemanticRoot::Documents),
        PathBuf::from("/Users/u/Documents")
    );
    assert_eq!(
        p.state_dir().unwrap(),
        support.join("Carrova").join("state")
    );
    assert_eq!(p.os(), OsKind::MacOs);
    assert_eq!(p.permission_model(), crate::PermissionModel::Unix);
}

#[test]
fn home_is_normalised_and_never_the_filesystem_root() {
    let p = MacPlatform::new(env_home("/Users/./u/x/.."));
    assert_eq!(root(&p, SemanticRoot::Home), PathBuf::from("/Users/u"));
    for bad in ["/", "/Users/..", "relative", ""] {
        let p = MacPlatform::new(env_home(bad));
        for r in [SemanticRoot::Home, SemanticRoot::UserConfig] {
            assert!(
                matches!(
                    p.resolve_root(r),
                    Err(PlatformError::RootUnavailable { .. })
                ),
                "HOME={bad:?} {r:?}"
            );
        }
        assert!(p.state_dir().is_err(), "HOME={bad:?}");
    }
    assert!(MacPlatform::new(MacEnv::default()).state_dir().is_err());
}

// ---- tools ----

#[test]
fn gnupg_home_and_tool_env() {
    let p = MacPlatform::new(MacEnv {
        home: Some("/Users/u".into()),
        path: Some("/opt/homebrew/bin:/usr/bin".into()),
    });
    assert_eq!(p.gnupg_home().unwrap(), PathBuf::from("/Users/u/.gnupg"));
    let cmd = p.tool_command(Path::new("/opt/homebrew/bin/gpg")).unwrap();
    let mut env: Vec<_> = cmd
        .get_envs()
        .map(|(k, v)| (k.to_os_string(), v.map(std::ffi::OsStr::to_os_string)))
        .collect();
    env.sort();
    assert_eq!(
        env,
        vec![
            ("HOME".into(), Some("/Users/u".into())),
            ("LC_ALL".into(), Some("C".into())),
            ("PATH".into(), Some("/opt/homebrew/bin:/usr/bin".into())),
        ]
    );
    let p = MacPlatform::new(MacEnv::default());
    assert!(p.gnupg_home().is_err());
    assert!(p.tool_command(Path::new("/usr/bin/gpg")).is_err());
}

#[test]
fn find_binary_scans_path_for_executables() {
    let tmp = TempDir::new().unwrap();
    let tool = tmp.path().join("tool");
    std::fs::write(&tool, b"#!/bin/sh\n").unwrap();
    let p = MacPlatform::new(MacEnv {
        home: None,
        path: Some(tmp.path().into()),
    });
    assert_eq!(p.find_binary("tool").unwrap(), None, "not executable");
    p.set_mode(&tool, 0o755).unwrap();
    assert_eq!(p.find_binary("tool").unwrap(), Some(tool));
    assert_eq!(p.find_binary("../tool").unwrap(), None);
    assert_eq!(
        MacPlatform::new(MacEnv::default())
            .find_binary("sh")
            .unwrap(),
        None
    );
}

// ---- system ----

#[test]
fn running_processes_include_this_test_untruncated() {
    let p = MacPlatform::from_env();
    let me = std::process::id();
    let procs = p.running_processes().unwrap();
    let this = procs
        .iter()
        .find(|pr| pr.pid == me)
        .expect("own process listed");
    let exe = std::env::current_exe().unwrap();
    let exe_name = exe.file_name().unwrap().to_str().unwrap();
    assert_eq!(this.name, exe_name);
    assert!(this.matches_binary(exe_name));
    assert!(procs.windows(2).all(|w| w[0].pid < w[1].pid));
}

#[test]
fn system_answers() {
    let tmp = TempDir::new().unwrap();
    let p = MacPlatform::from_env();
    assert!(p.free_space(tmp.path()).unwrap() > 0);
    assert_eq!(
        p.snapshot_capability(tmp.path()).unwrap(),
        SnapshotCapability::None
    );
    assert_eq!(p.current_uid(), Some(rustix::process::geteuid().as_raw()));
    if let Some(name) = p.device_name() {
        assert!(!name.ends_with(".local"));
    }
}

#[test]
fn udp_buffer_hint_names_maxsockbuf_with_cluster_overhead() {
    assert_eq!(maxsockbuf_for(8 << 20), 9 << 20);
    assert_eq!(maxsockbuf_for(1), 2);
    assert_eq!(maxsockbuf_for(usize::MAX), usize::MAX);
    let hint = MacPlatform::from_env().udp_buffer_hint(8 << 20).unwrap();
    assert!(hint.contains("kern.ipc.maxsockbuf=9437184"), "{hint}");
}

// ---- filesystem (macOS variants of the shared Unix primitives) ----

#[test]
fn rename_noreplace_refuses_an_existing_destination() {
    let tmp = TempDir::new().unwrap();
    let p = MacPlatform::from_env();
    let (a, b, c) = (
        tmp.path().join("a"),
        tmp.path().join("b"),
        tmp.path().join("c"),
    );
    std::fs::write(&a, b"A").unwrap();
    std::fs::write(&b, b"B").unwrap();
    let e = p.rename_noreplace(&a, &b).unwrap_err();
    assert_eq!(io_kind(&e), io::ErrorKind::AlreadyExists);
    assert_eq!(std::fs::read(&b).unwrap(), b"B");
    p.rename_noreplace(&a, &c).unwrap();
    assert_eq!(std::fs::read(&c).unwrap(), b"A");
}

#[test]
fn set_mode_and_create_dir_never_follow_a_symlink() {
    let tmp = TempDir::new().unwrap();
    let p = MacPlatform::from_env();
    let file = tmp.path().join("f");
    std::fs::write(&file, b"x").unwrap();
    p.set_mode(&file, 0o000).unwrap();
    p.set_mode(&file, 0o600).unwrap();
    assert_eq!(p.metadata(&file).unwrap().mode, Some(0o600));

    let link = tmp.path().join("l");
    p.create_symlink(file.to_str().unwrap(), &link).unwrap();
    let e = p.set_mode(&link, 0o777).unwrap_err();
    assert!(matches!(e, PlatformError::Io { .. }), "{e:?}");
    assert_eq!(p.metadata(&file).unwrap().mode, Some(0o600));

    let dir = tmp.path().join("d");
    p.create_dir(&dir, Some(0o700)).unwrap();
    assert_eq!(p.metadata(&dir).unwrap().mode, Some(0o700));
}

#[test]
fn sync_fs_is_unsupported_and_sync_dir_works() {
    let tmp = TempDir::new().unwrap();
    let p = MacPlatform::from_env();
    assert!(matches!(
        p.sync_fs(tmp.path()),
        Err(PlatformError::Unsupported("sync_fs"))
    ));
    p.sync_dir(tmp.path()).unwrap();
}

#[test]
fn preallocate_reserves_and_sizes_the_file() {
    let tmp = TempDir::new().unwrap();
    let p = MacPlatform::from_env();
    let f = p
        .create_new_file(&tmp.path().join("f"), Some(0o600))
        .unwrap();
    assert_eq!(
        p.preallocate(&f, 4096).unwrap(),
        crate::Allocation::Reserved
    );
    assert_eq!(f.metadata().unwrap().len(), 4096);
}

#[test]
fn lock_exclusive_blocks_a_second_locker_until_released() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("lock");
    let guard = MacPlatform::from_env().lock_exclusive(&path).unwrap();
    assert_eq!(
        MacPlatform::from_env().metadata(&path).unwrap().mode,
        Some(0o600)
    );
    let (tx, rx) = mpsc::channel();
    let waiter = {
        let path = path.clone();
        std::thread::spawn(move || {
            let _g = MacPlatform::from_env().lock_exclusive(&path).unwrap();
            tx.send(()).unwrap();
        })
    };
    assert!(rx.recv_timeout(Duration::from_millis(300)).is_err());
    drop(guard);
    rx.recv_timeout(Duration::from_secs(5)).unwrap();
    waiter.join().unwrap();
}
