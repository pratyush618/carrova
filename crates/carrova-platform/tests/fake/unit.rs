use super::*;
use crate::{OsKind, PermissionModel, Platform, ScreenLock, SnapshotCapability};

#[test]
fn fake_platform_configured_values() {
    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path();
    let p = FakePlatform::with_home(home)
        .root(SemanticRoot::Documents, home.join("Docs"))
        .binary("code", "/usr/bin/code")
        .process(42, "code")
        .free_space_bytes(1234);

    assert_eq!(p.os(), OsKind::Linux);
    assert_eq!(
        p.resolve_root(SemanticRoot::UserConfig).unwrap(),
        home.join(".config")
    );
    assert_eq!(
        p.resolve_root(SemanticRoot::Documents).unwrap(),
        home.join("Docs")
    );
    assert_eq!(
        p.find_binary("code").unwrap(),
        Some(PathBuf::from("/usr/bin/code"))
    );
    assert_eq!(p.find_binary("vim").unwrap(), None);
    let procs = p.running_processes().unwrap();
    assert_eq!(procs.len(), 1);
    assert_eq!((procs[0].pid, procs[0].name.as_str()), (42, "code"));
    assert_eq!(p.free_space(home).unwrap(), 1234);
    assert_eq!(
        p.snapshot_capability(home).unwrap(),
        SnapshotCapability::None
    );

    std::fs::write(home.join("f"), b"x").unwrap();
    let meta = p.metadata(&home.join("f")).unwrap();
    assert_eq!(meta.kind, crate::FileKind::File);
    assert_eq!(meta.size, 1);
}

#[test]
fn fake_defaults_to_a_linux_peer_with_unix_permissions() {
    let p = FakePlatform::with_home("/h");
    assert_eq!(p.os(), OsKind::Linux);
    let host_model = if cfg!(unix) {
        PermissionModel::Unix
    } else {
        PermissionModel::Acl
    };
    assert_eq!(p.permission_model(), host_model);
}

#[test]
fn fake_with_os_simulates_a_macos_or_windows_peer() {
    let mac = FakePlatform::with_home("/h").with_os(OsKind::MacOs);
    assert_eq!(mac.os(), OsKind::MacOs);
    #[cfg(unix)]
    assert_eq!(mac.permission_model(), PermissionModel::Unix);

    let win = FakePlatform::with_home("/h").with_os(OsKind::Windows);
    assert_eq!(win.os(), OsKind::Windows);
    assert_eq!(win.permission_model(), PermissionModel::Acl);
    assert_eq!(
        win.resolve_root(SemanticRoot::Home).unwrap(),
        PathBuf::from("/h"),
        "roots stay as configured"
    );
}

#[test]
fn fake_state_dir_default_and_override() {
    let p = FakePlatform::with_home("/h");
    assert_eq!(
        p.state_dir().unwrap(),
        PathBuf::from("/h/.local/state/carrova")
    );
    let p = p.with_state_dir("/elsewhere/st");
    assert_eq!(
        Platform::state_dir(&p).unwrap(),
        PathBuf::from("/elsewhere/st")
    );
}

#[cfg(unix)]
#[test]
fn fake_tool_script_and_env() {
    let tmp = tempfile::TempDir::new().unwrap();
    let p = FakePlatform::with_home(tmp.path());
    assert_eq!(p.gnupg_home().unwrap(), tmp.path().join(".gnupg"));
    let bin = write_tool_script(
        tmp.path(),
        "tool",
        "printf '%s|%s' \"$HOME\" \"$CARGO_MANIFEST_DIR\"\n",
    )
    .unwrap();
    assert_eq!(p.metadata(&bin).unwrap().mode, Some(0o755));
    let out = p.tool_command(&bin).unwrap().output().unwrap();
    assert!(out.status.success());
    // HOME is the fake home; inherited variables are gone.
    assert_eq!(
        String::from_utf8(out.stdout).unwrap(),
        format!("{}|", tmp.path().display())
    );
    assert!(write_tool_script(tmp.path(), "tool", "").is_err());
}

#[cfg(unix)]
#[test]
fn fake_os_layout_puts_roots_where_each_os_does() {
    let config = |os| {
        FakePlatform::with_os_layout("/h", os)
            .resolve_root(SemanticRoot::UserConfig)
            .unwrap()
    };
    assert_eq!(config(OsKind::Linux), PathBuf::from("/h/.config"));
    assert_eq!(
        config(OsKind::MacOs),
        PathBuf::from("/h/Library/Application Support")
    );
    let win = FakePlatform::with_os_layout("/h", OsKind::Windows);
    assert_eq!(win.os(), OsKind::Windows);
    assert_eq!(
        win.resolve_root(SemanticRoot::Cache).unwrap(),
        PathBuf::from("/h/AppData/Local")
    );
}

#[test]
fn fake_disk_names_the_matching_mount_and_configured_total() {
    let p = FakePlatform::with_home("/h")
        .disk_total_bytes(1000)
        .filesystem("/h/disk", 1, 500);
    let inside = p.disk(Path::new("/h/disk/x")).unwrap();
    assert_eq!(inside.total, 1000);
    assert_eq!(inside.mount_point, PathBuf::from("/h/disk"));
    assert_eq!(
        p.disk(Path::new("/h/other")).unwrap().mount_point,
        PathBuf::from("/")
    );
}

#[test]
fn fake_filesystems_override_fs_id_and_free_space() {
    let tmp = tempfile::TempDir::new().unwrap();
    let disk = tmp.path().join("disk");
    let inner = disk.join("inner");
    std::fs::create_dir_all(&inner).unwrap();
    let p = FakePlatform::with_home(tmp.path())
        .free_space_bytes(10)
        .filesystem(&disk, 1, 500)
        .filesystem(&inner, 2, 700);
    let real = p.fs_id(tmp.path()).unwrap();
    assert_ne!(p.fs_id(&disk).unwrap(), real);
    assert_ne!(p.fs_id(&inner).unwrap(), p.fs_id(&disk).unwrap());
    assert_eq!(p.fs_id(&disk.join("x")).unwrap(), p.fs_id(&disk).unwrap());
    assert_eq!(p.free_space(&disk).unwrap(), 500);
    assert_eq!(p.free_space(&inner.join("f")).unwrap(), 700);
    assert_eq!(p.free_space(tmp.path()).unwrap(), 10);
}

#[cfg(unix)]
#[test]
fn fake_preallocate_beyond_free_space_is_storage_full() {
    let tmp = tempfile::TempDir::new().unwrap();
    let p = FakePlatform::with_home(tmp.path()).free_space_bytes(100);
    let f = p
        .create_new_file(&tmp.path().join("f"), Some(0o600))
        .unwrap();
    let e = p.preallocate(&f, 101).unwrap_err();
    assert_eq!(e.kind(), std::io::ErrorKind::StorageFull);
    assert_eq!(p.preallocate(&f, 100).unwrap(), crate::Allocation::Reserved);
    assert_eq!(f.metadata().unwrap().len(), 100);
}

#[cfg(unix)]
#[test]
fn fake_fs_ops_are_real() {
    let tmp = tempfile::TempDir::new().unwrap();
    let p = FakePlatform::with_home(tmp.path());
    let st = Platform::state_dir(&p).unwrap();
    std::fs::create_dir_all(st.parent().unwrap()).unwrap();
    p.create_dir(&st, Some(0o700)).unwrap();
    assert_eq!(p.metadata(&st).unwrap().mode, Some(0o700));
    let f = st.join("session.json");
    drop(p.create_new_file(&f, Some(0o600)).unwrap());
    assert_eq!(p.metadata(&f).unwrap().mode, Some(0o600));
    p.sync_dir(&st).unwrap();
    p.remove_dir_all_confined(&st, tmp.path()).unwrap();
    assert!(!st.exists());
}

#[cfg(unix)]
mod hooks {
    use std::sync::Mutex;

    use super::*;
    use crate::PlatformError;

    fn io_denied(path: &Path) -> PlatformError {
        PlatformError::io(path, std::io::ErrorKind::PermissionDenied.into())
    }

    #[test]
    fn rename_hook_error_replaces_the_rename() {
        let tmp = tempfile::TempDir::new().unwrap();
        let (a, b) = (tmp.path().join("a"), tmp.path().join("b"));
        std::fs::write(&a, b"x").unwrap();
        let p = FakePlatform::with_home(tmp.path()).with_hooks(Hooks {
            on_rename: Some(Arc::new(|_, to| Err(io_denied(to)))),
            ..Hooks::default()
        });
        assert!(p.rename_noreplace(&a, &b).is_err());
        assert!(a.exists() && !b.exists());
    }

    #[test]
    fn sync_dir_hook_error_replaces_the_sync() {
        let tmp = tempfile::TempDir::new().unwrap();
        let p = FakePlatform::with_home(tmp.path()).with_hooks(Hooks {
            on_sync_dir: Some(Arc::new(|dir| Err(io_denied(dir)))),
            ..Hooks::default()
        });
        assert!(p.sync_dir(tmp.path()).is_err());
    }

    #[test]
    fn open_hook_runs_after_a_successful_open_only() {
        let tmp = tempfile::TempDir::new().unwrap();
        let f = tmp.path().join("f");
        std::fs::write(&f, b"x").unwrap();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&seen);
        let p = FakePlatform::with_home(tmp.path()).with_hooks(Hooks {
            on_open: Some(Arc::new(move |path| {
                log.lock().unwrap().push(path.to_path_buf());
            })),
            ..Hooks::default()
        });
        drop(p.open_read_nofollow(&f).unwrap());
        assert!(p.open_read_nofollow(&tmp.path().join("missing")).is_err());
        assert_eq!(*seen.lock().unwrap(), [f]);
    }

    #[test]
    fn metadata_hooks_rewrite_reports() {
        let tmp = tempfile::TempDir::new().unwrap();
        let f = tmp.path().join("f");
        std::fs::write(&f, b"x").unwrap();
        let p = FakePlatform::with_home(tmp.path()).with_hooks(Hooks {
            on_metadata: Some(Arc::new(|_, m| m.size = 99)),
            hide_mode: true,
            uid: Some(4242),
            ..Hooks::default()
        });
        let meta = p.metadata(&f).unwrap();
        assert_eq!((meta.size, meta.mode), (99, None));
        let fd_meta = p.file_metadata(&std::fs::File::open(&f).unwrap()).unwrap();
        assert_eq!((fd_meta.size, fd_meta.mode), (1, None));
        assert_eq!(p.current_uid(), Some(4242));
    }

    #[test]
    fn free_space_override_and_dir_sync_count_are_shared_by_clones_only() {
        let tmp = tempfile::TempDir::new().unwrap();
        let plain = FakePlatform::with_home(tmp.path()).free_space_bytes(10);
        let hooked = plain.clone().with_hooks(Hooks::default());
        let clone = hooked.clone();
        clone.override_free_space(Some(7));
        assert_eq!(hooked.free_space(tmp.path()).unwrap(), 7);
        assert_eq!(plain.free_space(tmp.path()).unwrap(), 10);
        hooked.override_free_space(None);
        assert_eq!(clone.free_space(tmp.path()).unwrap(), 10);

        clone.sync_dir(tmp.path()).unwrap();
        assert_eq!((hooked.dir_syncs(), plain.dir_syncs()), (1, 0));
    }
}

#[test]
fn fake_device_name_defaults_to_none() {
    let tmp = tempfile::TempDir::new().unwrap();
    assert_eq!(FakePlatform::with_home(tmp.path()).device_name(), None);
}

#[test]
fn fake_device_name_is_sanitised_like_a_native_one() {
    let tmp = tempfile::TempDir::new().unwrap();
    let named = |raw: &str| FakePlatform::with_home(tmp.path()).with_device_name(raw);
    assert_eq!(named("  studio\n").device_name().as_deref(), Some("studio"));
    assert_eq!(named("\u{202E}").device_name(), None);
}

/// Scripts written while other threads fork must run at once (no ETXTBSY).
#[cfg(unix)]
#[test]
fn tool_scripts_run_at_once_while_other_threads_fork() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let tmp = tempfile::TempDir::new().unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let forkers: Vec<_> = (0..4)
        .map(|_| {
            let stop = Arc::clone(&stop);
            std::thread::spawn(move || {
                while !stop.load(Ordering::Relaxed) {
                    let _ = std::process::Command::new("/bin/true").status();
                }
            })
        })
        .collect();
    let writers: Vec<_> = (0..4)
        .map(|t| {
            let dir = tmp.path().to_owned();
            std::thread::spawn(move || {
                for i in 0..50 {
                    let bin =
                        write_tool_script(&dir, &format!("tool-{t}-{i}"), "exit 0\n").unwrap();
                    let status = std::process::Command::new(&bin).status().unwrap();
                    assert!(status.success());
                }
            })
        })
        .collect();
    for w in writers {
        w.join().unwrap();
    }
    stop.store(true, Ordering::Relaxed);
    for f in forkers {
        f.join().unwrap();
    }
}

#[test]
fn fake_counts_display_wakes_and_reports_the_lock_the_test_sets() {
    let tmp = tempfile::TempDir::new().unwrap();
    let fake = FakePlatform::with_home(tmp.path());
    assert_eq!(fake.screen_lock(), ScreenLock::Unknown);
    fake.wake_display().unwrap();
    fake.clone().wake_display().unwrap();
    assert_eq!(fake.wakes(), 2);
    fake.set_screen_lock(ScreenLock::Locked);
    assert_eq!(fake.screen_lock(), ScreenLock::Locked);
}

#[test]
fn adapters_without_a_display_to_wake_say_so_and_never_report_a_lock() {
    #[derive(Debug)]
    struct Bare;
    impl Platform for Bare {
        fn os(&self) -> OsKind {
            OsKind::Linux
        }
        fn resolve_root(&self, _: SemanticRoot) -> Result<PathBuf, crate::PlatformError> {
            unreachable!()
        }
        fn find_binary(&self, _: &str) -> Result<Option<PathBuf>, crate::PlatformError> {
            unreachable!()
        }
        fn metadata(&self, _: &Path) -> Result<crate::FileMeta, crate::PlatformError> {
            unreachable!()
        }
        fn running_processes(&self) -> Result<Vec<ProcessInfo>, crate::PlatformError> {
            unreachable!()
        }
        fn free_space(&self, _: &Path) -> Result<u64, crate::PlatformError> {
            unreachable!()
        }
        fn snapshot_capability(
            &self,
            _: &Path,
        ) -> Result<SnapshotCapability, crate::PlatformError> {
            unreachable!()
        }
    }
    assert!(matches!(
        Bare.wake_display(),
        Err(crate::PlatformError::Unsupported(_))
    ));
    assert_eq!(Bare.screen_lock(), ScreenLock::Unknown);
}
