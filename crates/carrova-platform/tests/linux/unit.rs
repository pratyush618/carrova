use super::*;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use tempfile::TempDir;

fn env_home(home: &str) -> LinuxEnv {
    LinuxEnv {
        home: Some(home.into()),
        ..LinuxEnv::default()
    }
}

fn root(p: &LinuxPlatform, r: SemanticRoot) -> PathBuf {
    p.resolve_root(r).unwrap()
}

#[test]
fn file_metadata_describes_the_opened_file_not_the_path() {
    let tmp = TempDir::new().unwrap();
    let p = LinuxPlatform::new(env_home("/home/u"));
    let path = tmp.path().join("key");
    fs::write(&path, b"opened").unwrap();
    p.set_mode(&path, 0o644).unwrap();
    let file = p.open_read_nofollow(&path).unwrap();
    let other = tmp.path().join("other");
    fs::write(&other, b"swapped in").unwrap();
    p.set_mode(&other, 0o600).unwrap();
    fs::rename(&other, &path).unwrap();

    let meta = p.file_metadata(&file).unwrap();
    assert_eq!(meta.mode, Some(0o644));
    assert_eq!(meta.kind, crate::FileKind::File);
    assert_eq!(meta.size, 6);
    assert_eq!(meta.uid, p.current_uid());
    assert_eq!(p.metadata(&path).unwrap().mode, Some(0o600));
}

// ---- roots ----

#[test]
fn gnupg_home_and_tool_env() {
    let p = LinuxPlatform::new(LinuxEnv {
        home: Some("/h/u".into()),
        path: Some("/usr/bin:/bin".into()),
        ..LinuxEnv::default()
    });
    assert_eq!(p.gnupg_home().unwrap(), PathBuf::from("/h/u/.gnupg"));
    let cmd = p.tool_command(Path::new("/usr/bin/gpg")).unwrap();
    let mut env: Vec<(String, Option<String>)> = cmd
        .get_envs()
        .map(|(k, v)| {
            (
                k.to_string_lossy().into_owned(),
                v.map(|v| v.to_string_lossy().into_owned()),
            )
        })
        .collect();
    env.sort();
    assert_eq!(
        env,
        vec![
            ("HOME".into(), Some("/h/u".into())),
            ("LC_ALL".into(), Some("C".into())),
            ("PATH".into(), Some("/usr/bin:/bin".into())),
        ]
    );
    assert_eq!(cmd.get_program(), "/usr/bin/gpg");
    // No HOME → no gnupg home, no tool.
    let p = LinuxPlatform::new(LinuxEnv::default());
    assert!(p.gnupg_home().is_err());
    assert!(p.tool_command(Path::new("/usr/bin/gpg")).is_err());
}

#[test]
fn default_roots() {
    let p = LinuxPlatform::new(env_home("/home/u"));
    assert_eq!(root(&p, SemanticRoot::Home), PathBuf::from("/home/u"));
    assert_eq!(
        root(&p, SemanticRoot::UserConfig),
        PathBuf::from("/home/u/.config")
    );
    assert_eq!(
        root(&p, SemanticRoot::UserData),
        PathBuf::from("/home/u/.local/share")
    );
    assert_eq!(
        root(&p, SemanticRoot::Cache),
        PathBuf::from("/home/u/.cache")
    );
    assert_eq!(
        root(&p, SemanticRoot::Documents),
        PathBuf::from("/home/u/Documents")
    );
}

#[test]
fn xdg_overrides() {
    let p = LinuxPlatform::new(LinuxEnv {
        home: Some("/home/u".into()),
        xdg_config_home: Some("/cfg".into()),
        xdg_data_home: Some("/data".into()),
        xdg_cache_home: Some("/cache".into()),
        xdg_state_home: None,
        path: None,
        appimage: None,
    });
    assert_eq!(root(&p, SemanticRoot::UserConfig), PathBuf::from("/cfg"));
    assert_eq!(root(&p, SemanticRoot::UserData), PathBuf::from("/data"));
    assert_eq!(root(&p, SemanticRoot::Cache), PathBuf::from("/cache"));
}

#[test]
fn relative_or_empty_xdg_ignored() {
    let p = LinuxPlatform::new(LinuxEnv {
        home: Some("/home/u".into()),
        xdg_config_home: Some("rel/cfg".into()),
        xdg_data_home: Some("".into()),
        xdg_cache_home: Some("./c".into()),
        xdg_state_home: None,
        path: None,
        appimage: None,
    });
    assert_eq!(
        root(&p, SemanticRoot::UserConfig),
        PathBuf::from("/home/u/.config")
    );
    assert_eq!(
        root(&p, SemanticRoot::UserData),
        PathBuf::from("/home/u/.local/share")
    );
    assert_eq!(
        root(&p, SemanticRoot::Cache),
        PathBuf::from("/home/u/.cache")
    );
}

fn state_env(home: Option<&str>, xdg: Option<&str>) -> LinuxPlatform {
    LinuxPlatform::new(LinuxEnv {
        home: home.map(Into::into),
        xdg_state_home: xdg.map(Into::into),
        ..LinuxEnv::default()
    })
}

#[test]
fn state_dir_env_cases() {
    let ok = |home, xdg, want: &str| {
        assert_eq!(
            state_env(home, xdg).state_dir().unwrap(),
            PathBuf::from(want),
            "home={home:?} xdg={xdg:?}"
        );
    };
    ok(Some("/home/u"), None, "/home/u/.local/state/carrova");
    ok(Some("/home/u"), Some("/var/st"), "/var/st/carrova");
    ok(Some("/home/u"), Some("/var/./x/../st/"), "/var/st/carrova");
    ok(None, Some("/st"), "/st/carrova");
    // Empty or relative XDG value: ignored per the XDG spec.
    ok(Some("/home/u"), Some(""), "/home/u/.local/state/carrova");
    ok(
        Some("/home/u"),
        Some("rel/st"),
        "/home/u/.local/state/carrova",
    );
    ok(Some("/home/./u/"), None, "/home/u/.local/state/carrova");

    for xdg in ["/", "/..", "/a/..", "/../st"] {
        assert!(
            matches!(
                state_env(Some("/home/u"), Some(xdg)).state_dir(),
                Err(PlatformError::StateDirUnavailable(_))
            ),
            "{xdg}"
        );
    }
    for home in [None, Some(""), Some("rel"), Some("/")] {
        assert!(
            matches!(
                state_env(home, None).state_dir(),
                Err(PlatformError::RootUnavailable { .. })
            ),
            "{home:?}"
        );
    }
}

#[test]
fn missing_or_relative_home() {
    for env in [LinuxEnv::default(), env_home(""), env_home("relative/home")] {
        let p = LinuxPlatform::new(env);
        for r in [
            SemanticRoot::Home,
            SemanticRoot::UserConfig,
            SemanticRoot::Documents,
        ] {
            assert!(matches!(
                p.resolve_root(r),
                Err(PlatformError::RootUnavailable { .. })
            ));
        }
    }
}

fn assert_unavailable(p: &LinuxPlatform, r: SemanticRoot, ctx: &str) {
    let got = p.resolve_root(r);
    assert!(
        matches!(got, Err(PlatformError::RootUnavailable { root, .. }) if root == r),
        "{ctx}: {r} resolved to {got:?}"
    );
}

#[test]
fn filesystem_root_is_never_a_semantic_root() {
    // XDG vars pointing at `/` (directly or via `..`) are rejected.
    for v in [
        "/",
        "//",
        "/.",
        "/home/..",
        "/a/b/../../",
        "/../etc",
        "/a/../../b",
    ] {
        let p = LinuxPlatform::new(LinuxEnv {
            home: Some("/home/u".into()),
            xdg_config_home: Some(v.into()),
            xdg_data_home: Some(v.into()),
            xdg_cache_home: Some(v.into()),
            xdg_state_home: None,
            path: None,
            appimage: None,
        });
        for r in [
            SemanticRoot::UserConfig,
            SemanticRoot::UserData,
            SemanticRoot::Cache,
        ] {
            assert_unavailable(&p, r, v);
        }
        assert_eq!(root(&p, SemanticRoot::Home), PathBuf::from("/home/u"));
    }
    // HOME=/ (or something normalising to it) poisons every derived root.
    for h in ["/", "/home/u/../..", "/../home"] {
        let p = LinuxPlatform::new(env_home(h));
        for r in SemanticRoot::ALL {
            // Derived roots report the underlying cause (`home`).
            assert!(
                matches!(
                    p.resolve_root(r),
                    Err(PlatformError::RootUnavailable { .. })
                ),
                "{h}: {r}"
            );
        }
    }
}

#[test]
fn roots_are_lexically_normalised() {
    let p = LinuxPlatform::new(LinuxEnv {
        home: Some("/home/./u/".into()),
        xdg_config_home: Some("/srv/x/../cfg/.".into()),
        ..LinuxEnv::default()
    });
    assert_eq!(root(&p, SemanticRoot::Home), PathBuf::from("/home/u"));
    assert_eq!(
        root(&p, SemanticRoot::UserConfig),
        PathBuf::from("/srv/cfg")
    );
    assert_eq!(
        root(&p, SemanticRoot::Cache),
        PathBuf::from("/home/u/.cache")
    );
}

#[test]
fn documents_dir_resolving_to_filesystem_root_rejected() {
    for v in [
        "XDG_DOCUMENTS_DIR=\"/\"",
        "XDG_DOCUMENTS_DIR=\"$HOME/../..\"",
        "XDG_DOCUMENTS_DIR=\"${HOME}/../../\"",
        "XDG_DOCUMENTS_DIR=\"/../srv\"",
    ] {
        let tmp = TempDir::new().unwrap();
        let cfg = tmp.path().join("cfg");
        fs::create_dir_all(&cfg).unwrap();
        fs::write(cfg.join("user-dirs.dirs"), v).unwrap();
        let p = LinuxPlatform::new(LinuxEnv {
            home: Some("/home/u".into()),
            xdg_config_home: Some(cfg.into_os_string()),
            ..LinuxEnv::default()
        });
        assert_unavailable(&p, SemanticRoot::Documents, v);
    }
    assert_eq!(
        docs_with("XDG_DOCUMENTS_DIR=\"$HOME/../shared/docs\""),
        PathBuf::from("/home/shared/docs")
    );
}

#[test]
fn absolute_xdg_works_without_home() {
    let p = LinuxPlatform::new(LinuxEnv {
        xdg_config_home: Some("/cfg".into()),
        ..LinuxEnv::default()
    });
    assert_eq!(root(&p, SemanticRoot::UserConfig), PathBuf::from("/cfg"));
}

fn docs_with(user_dirs: &str) -> PathBuf {
    let tmp = TempDir::new().unwrap();
    let cfg = tmp.path().join("cfg");
    fs::create_dir_all(&cfg).unwrap();
    fs::write(cfg.join("user-dirs.dirs"), user_dirs).unwrap();
    let p = LinuxPlatform::new(LinuxEnv {
        home: Some("/home/u".into()),
        xdg_config_home: Some(cfg.into_os_string()),
        ..LinuxEnv::default()
    });
    root(&p, SemanticRoot::Documents)
}

#[test]
fn user_dirs_parsing_variants() {
    let cases = [
        (
            "# comment\nXDG_DESKTOP_DIR=\"$HOME/Desktop\"\nXDG_DOCUMENTS_DIR=\"$HOME/Docs\"\n",
            "/home/u/Docs",
        ),
        ("XDG_DOCUMENTS_DIR=\"/srv/docs\"", "/srv/docs"),
        (
            "  XDG_DOCUMENTS_DIR = \"${HOME}/My Docs\"  # trailing",
            "/home/u/My Docs",
        ),
        ("XDG_DOCUMENTS_DIR=$HOME/bare", "/home/u/bare"),
        ("XDG_DOCUMENTS_DIR='$HOME/single'", "/home/u/single"),
        ("XDG_DOCUMENTS_DIR=\"$HOME\"", "/home/u"),
        (
            "XDG_DOCUMENTS_DIR=\"$HOME/Say \\\"hi\\\"\"",
            "/home/u/Say \"hi\"",
        ),
        // last assignment wins
        (
            "XDG_DOCUMENTS_DIR=\"$HOME/a\"\nXDG_DOCUMENTS_DIR=\"$HOME/b\"",
            "/home/u/b",
        ),
        // commented-out line ignored
        ("#XDG_DOCUMENTS_DIR=\"$HOME/x\"", "/home/u/Documents"),
        // relative value is invalid → fallback
        ("XDG_DOCUMENTS_DIR=\"Docs\"", "/home/u/Documents"),
        // similar key prefix not matched
        ("XDG_DOCUMENTS_DIRX=\"/nope\"", "/home/u/Documents"),
        ("", "/home/u/Documents"),
    ];
    for (input, want) in cases {
        assert_eq!(docs_with(input), PathBuf::from(want), "input: {input:?}");
    }
}

// ---- find_binary ----

fn write_file(path: &Path, mode: u32) {
    fs::write(path, b"#!/bin/sh\n").unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
}

#[test]
fn find_binary_respects_exec_bit_and_order() {
    let tmp = TempDir::new().unwrap();
    let a = tmp.path().join("a");
    let b = tmp.path().join("b");
    fs::create_dir_all(&a).unwrap();
    fs::create_dir_all(&b).unwrap();
    write_file(&a.join("tool"), 0o644); // not executable
    write_file(&b.join("tool"), 0o755);
    write_file(&a.join("first"), 0o700);
    write_file(&b.join("first"), 0o755);
    fs::create_dir(a.join("dir")).unwrap();
    fs::set_permissions(a.join("dir"), fs::Permissions::from_mode(0o755)).unwrap();
    std::os::unix::fs::symlink(b.join("tool"), a.join("link")).unwrap();

    let path =
        std::env::join_paths(["", "relative", a.to_str().unwrap(), b.to_str().unwrap()]).unwrap();
    let p = LinuxPlatform::new(LinuxEnv {
        path: Some(path),
        ..LinuxEnv::default()
    });
    assert_eq!(p.find_binary("tool").unwrap(), Some(b.join("tool")));
    assert_eq!(p.find_binary("first").unwrap(), Some(a.join("first")));
    assert_eq!(p.find_binary("link").unwrap(), Some(a.join("link")));
    assert_eq!(p.find_binary("dir").unwrap(), None);
    assert_eq!(p.find_binary("missing").unwrap(), None);
    assert_eq!(p.find_binary("").unwrap(), None);
    assert_eq!(p.find_binary("../b/tool").unwrap(), None);
}

#[test]
fn find_binary_without_path() {
    let p = LinuxPlatform::new(LinuxEnv::default());
    assert_eq!(p.find_binary("sh").unwrap(), None);
}

// ---- processes ----

#[test]
fn processes_from_fake_proc() {
    let tmp = TempDir::new().unwrap();
    let proc_root = tmp.path();
    let mk = |pid: &str, comm: Option<&str>, exe: Option<&str>| {
        let d = proc_root.join(pid);
        fs::create_dir_all(&d).unwrap();
        if let Some(c) = comm {
            fs::write(d.join("comm"), c).unwrap();
        }
        if let Some(e) = exe {
            std::os::unix::fs::symlink(e, d.join("exe")).unwrap();
        }
    };
    mk("42", Some("firefox\n"), Some("/usr/lib/firefox/firefox"));
    mk("7", Some("kthreadd\n"), None);
    mk("99", None, None); // vanished: no comm
    mk("self", Some("x\n"), None); // non-numeric
    fs::write(proc_root.join("uptime"), "1 2").unwrap();

    let p = LinuxPlatform::new(LinuxEnv::default()).with_proc_root(proc_root);
    let procs = p.running_processes().unwrap();
    assert_eq!(
        procs,
        vec![
            ProcessInfo {
                pid: 7,
                name: "kthreadd".into(),
                exe: None
            },
            ProcessInfo {
                pid: 42,
                name: "firefox".into(),
                exe: Some("/usr/lib/firefox/firefox".into())
            },
        ]
    );
}

#[test]
fn processes_missing_proc_root_is_error() {
    let tmp = TempDir::new().unwrap();
    let p = LinuxPlatform::new(LinuxEnv::default()).with_proc_root(tmp.path().join("nope"));
    assert!(matches!(
        p.running_processes(),
        Err(PlatformError::Io { .. })
    ));
}

#[test]
fn real_proc_contains_self() {
    let p = LinuxPlatform::from_env();
    let me = std::process::id();
    let procs = p.running_processes().unwrap();
    let mine = procs
        .iter()
        .find(|pr| pr.pid == me)
        .expect("own pid listed");
    assert!(!mine.name.is_empty());
    assert!(mine.exe.is_some());
}

// ---- free space ----

#[test]
fn free_space_on_tempdir() {
    let tmp = TempDir::new().unwrap();
    let p = LinuxPlatform::new(LinuxEnv::default());
    assert!(p.free_space(tmp.path()).unwrap() > 0);
}

#[test]
fn free_space_missing_path_is_error() {
    let tmp = TempDir::new().unwrap();
    let p = LinuxPlatform::new(LinuxEnv::default());
    assert!(matches!(
        p.free_space(&tmp.path().join("nope")),
        Err(PlatformError::Io { .. })
    ));
}

#[test]
fn disk_names_the_mount_holding_a_path_and_its_capacity() {
    use std::os::unix::fs::MetadataExt;
    let tmp = TempDir::new().unwrap();
    let p = LinuxPlatform::new(LinuxEnv::default());
    let dev = |path: &Path| fs::metadata(path).unwrap().dev();
    let disk = p.disk(tmp.path()).unwrap();
    assert!(disk.total >= p.free_space(tmp.path()).unwrap());
    let real = fs::canonicalize(tmp.path()).unwrap();
    assert!(real.starts_with(&disk.mount_point), "{disk:?}");
    assert_eq!(dev(&disk.mount_point), dev(&real));
    if let Some(parent) = disk.mount_point.parent() {
        assert_ne!(dev(parent), dev(&disk.mount_point), "{disk:?}");
    }
    assert_eq!(p.disk(Path::new("/")).unwrap().mount_point, Path::new("/"));
    assert!(p.disk(&tmp.path().join("nope")).is_err());
}

#[test]
fn device_name_is_the_sanitised_kernel_host_name() {
    let kernel = fs::read_to_string("/proc/sys/kernel/hostname").unwrap();
    let p = LinuxPlatform::new(env_home("/home/u"));
    assert_eq!(p.device_name(), Some(kernel.trim().to_owned()));
}
