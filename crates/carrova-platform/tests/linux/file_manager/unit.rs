use super::*;
use crate::linux::{LinuxEnv, LinuxPlatform};
use crate::{FileManagerStatus, Platform};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

const NASTY: &str = "/opt/my apps/it's \"q\" $HOME `x` \\b;c%d/carrova";

struct Fixture {
    tmp: TempDir,
    platform: LinuxPlatform,
}

impl Fixture {
    fn new() -> Self {
        Self::with_appimage(None)
    }

    fn with_appimage(appimage: Option<&str>) -> Self {
        let tmp = TempDir::new().unwrap();
        let platform = LinuxPlatform::new(LinuxEnv {
            home: Some(tmp.path().into()),
            xdg_data_home: Some(tmp.path().join("data").into()),
            xdg_config_home: Some(tmp.path().join("config").into()),
            appimage: appimage.map(Into::into),
            ..LinuxEnv::default()
        });
        Self { tmp, platform }
    }

    fn data(&self, rel: &str) -> PathBuf {
        self.tmp.path().join("data").join(rel)
    }

    fn uca(&self) -> PathBuf {
        self.tmp.path().join("config/Thunar/uca.xml")
    }

    fn read(&self, path: &Path) -> String {
        fs::read_to_string(path).unwrap()
    }

    fn mode(&self, path: &Path) -> u32 {
        fs::metadata(path).unwrap().permissions().mode() & 0o777
    }
}

const ENTRY_FILES: [&str; 5] = [
    "nautilus-python/extensions/carrova_send.py",
    "nautilus/scripts/Send with Carrova",
    "nemo/actions/carrova-send.nemo_action",
    "kio/servicemenus/carrova-send.desktop",
    "file-manager/actions/carrova-send.desktop",
];

const USER_UCA: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<actions>
<action>
\t<icon>utilities-terminal</icon>
\t<name>Open Terminal Here</name>
\t<unique-id>1700000000000000-1</unique-id>
\t<command>exo-open --working-directory %f --launch TerminalEmulator</command>
\t<description>Example for a custom action</description>
\t<patterns>*</patterns>
\t<startup-notify/>
\t<directories/>
</action>
</actions>
";

// ---- quoting ----

#[test]
fn exec_quoting_escapes_spaces_and_quotes() {
    assert_eq!(exec_quote("/usr/bin/carrova"), "\"/usr/bin/carrova\"");
    // Exec rule: `"` `` ` `` `$` `\` get a backslash, `%` doubles; then the
    // key-file string rule doubles every backslash.
    assert_eq!(
        exec_quote(NASTY),
        "\"/opt/my apps/it's \\\\\"q\\\\\" \\\\$HOME \\\\`x\\\\` \\\\\\\\b;c%%d/carrova\""
    );
}

#[test]
fn key_file_values_escape_backslash_and_list_separator() {
    assert_eq!(key_string("/a\\b;c"), "/a\\\\b;c");
    assert_eq!(key_list("/a\\b;c"), "/a\\\\b\\;c;");
}

#[test]
fn py_string_escapes_quotes_backslashes_and_controls() {
    assert_eq!(
        py_string("/a \"b\" \\c\t'd'"),
        "\"/a \\\"b\\\" \\\\c\\x09'd'\""
    );
}

#[test]
fn script_runs_launcher_with_nasty_path_and_forwards_paths() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().join("it's \"q\" $HOME `x` \\b;c%d");
    fs::create_dir_all(&dir).unwrap();
    let launcher = dir.join("carrova");
    let out = tmp.path().join("argv");
    fs::write(
        &launcher,
        format!("#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}'\n", out.display()),
    )
    .unwrap();
    fs::set_permissions(&launcher, fs::Permissions::from_mode(0o755)).unwrap();
    let script = tmp.path().join("script");
    fs::write(&script, render(NAUTILUS_SCRIPT, launcher.to_str().unwrap())).unwrap();

    let status = std::process::Command::new("sh")
        .arg(&script)
        .args(["a b", "$c"])
        .status()
        .unwrap();
    assert!(status.success());
    assert_eq!(fs::read_to_string(&out).unwrap(), "--send\na b\n$c\n");
}

#[test]
fn script_exits_quietly_when_launcher_is_gone() {
    let tmp = TempDir::new().unwrap();
    let script = tmp.path().join("script");
    fs::write(&script, render(NAUTILUS_SCRIPT, "/nonexistent/carrova")).unwrap();
    let status = std::process::Command::new("sh")
        .arg(&script)
        .status()
        .unwrap();
    assert!(status.success());
}

#[test]
fn thunar_command_survives_the_shell_with_nasty_path() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().join("it's \"q\" $HOME `x` \\b;c");
    fs::create_dir_all(&dir).unwrap();
    let launcher = dir.join("carrova");
    let out = tmp.path().join("argv");
    fs::write(
        &launcher,
        format!("#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}'\n", out.display()),
    )
    .unwrap();
    fs::set_permissions(&launcher, fs::Permissions::from_mode(0o755)).unwrap();
    // Thunar replaces %F with the quoted selection, then splits like a shell.
    let command = thunar_command(launcher.to_str().unwrap()).replace("%F", "'a b' 'c'");

    let status = std::process::Command::new("sh")
        .args(["-c", &command])
        .status()
        .unwrap();
    assert!(status.success());
    assert_eq!(fs::read_to_string(&out).unwrap(), "--send\na b\nc\n");
}

#[test]
fn templates_have_no_unfilled_placeholders() {
    for template in [
        NAUTILUS_EXTENSION,
        NAUTILUS_SCRIPT,
        NEMO_ACTION,
        DOLPHIN_MENU,
        FILE_MANAGER_ACTION,
        THUNAR_ACTION,
    ] {
        assert!(!render(template, "/x").contains("{{"), "{template}");
    }
}

#[test]
fn render_fills_placeholders_once_even_when_the_path_looks_like_one() {
    assert_eq!(render("L={{sh}}", "/x/{{py}}"), "L='/x/{{py}}'");
}

// ---- launcher ----

#[test]
fn appimage_env_wins_over_current_exe() {
    let f = Fixture::with_appimage(Some("/home/u/Apps/Carrova.AppImage"));
    assert_eq!(
        f.platform.launcher().unwrap(),
        PathBuf::from("/home/u/Apps/Carrova.AppImage")
    );
    let plain = Fixture::new();
    assert_eq!(
        plain.platform.launcher().unwrap(),
        std::env::current_exe().unwrap()
    );
}

#[test]
fn launcher_with_newline_percent_or_relative_path_is_unsupported() {
    let f = Fixture::new();
    // `%`: not every file manager un-doubles `%%` in `Exec` (Nemo).
    for bad in [
        "/opt/car\nrova",
        "carrova",
        "/opt/car\rrova",
        "/opt/100%/carrova",
    ] {
        let err = f
            .platform
            .install_file_manager_entries(Path::new(bad))
            .unwrap_err();
        assert!(
            matches!(err, PlatformError::Unsupported("launcher path")),
            "{bad:?}"
        );
    }
    assert!(!f.tmp.path().join("data").exists());
}

// ---- install / remove / status ----

#[test]
fn install_writes_every_entry_pointing_at_launcher() {
    let f = Fixture::new();
    let status = f
        .platform
        .install_file_manager_entries(Path::new("/opt/carrova/carrova"))
        .unwrap();
    assert_eq!(
        status,
        FileManagerStatus {
            supported: true,
            installed: true,
            skipped: vec![],
        }
    );
    for rel in ENTRY_FILES {
        let text = f.read(&f.data(rel));
        assert!(text.contains("/opt/carrova/carrova"), "{rel}");
        assert!(text.contains("--send"), "{rel}");
    }
    let uca = f.read(&f.uca());
    assert!(uca.contains("<unique-id>carrova-send-with</unique-id>"));
    assert!(uca.contains("&apos;/opt/carrova/carrova&apos; %F"));

    let dolphin = f.read(&f.data("kio/servicemenus/carrova-send.desktop"));
    assert!(dolphin.contains("Exec=\"/opt/carrova/carrova\" --send %F\n"));
    assert!(dolphin.contains("TryExec=/opt/carrova/carrova\n"));
    assert!(!f.data("applications").exists(), "no Open With entry");
    let nemo = f.read(&f.data("nemo/actions/carrova-send.nemo_action"));
    assert!(nemo.contains("Dependencies=/opt/carrova/carrova;\n"));
    assert!(nemo.contains("Selection=notnone\n"));
    let py = f.read(&f.data("nautilus-python/extensions/carrova_send.py"));
    assert!(py.contains("LAUNCHER = \"/opt/carrova/carrova\"\n"));

    assert_eq!(f.mode(&f.data("nautilus/scripts/Send with Carrova")), 0o755);
    assert_eq!(
        f.mode(&f.data("kio/servicemenus/carrova-send.desktop")),
        0o755
    );
}

#[test]
fn install_refreshes_a_moved_launcher_without_leftovers() {
    let f = Fixture::new();
    f.platform
        .install_file_manager_entries(Path::new("/old/carrova"))
        .unwrap();
    f.platform
        .install_file_manager_entries(Path::new("/new/carrova"))
        .unwrap();
    for rel in ENTRY_FILES {
        let path = f.data(rel);
        assert!(!f.read(&path).contains("/old/"), "{rel}");
        let dir = fs::read_dir(path.parent().unwrap()).unwrap().count();
        assert_eq!(dir, 1, "temp file left beside {rel}");
    }
    assert!(!f.read(&f.uca()).contains("/old/"));
}

#[test]
fn remove_deletes_only_our_files() {
    let f = Fixture::new();
    let neighbour = f.data("nemo/actions/user.nemo_action");
    fs::create_dir_all(neighbour.parent().unwrap()).unwrap();
    fs::write(&neighbour, "mine").unwrap();
    fs::create_dir_all(f.uca().parent().unwrap()).unwrap();
    fs::write(f.uca(), USER_UCA).unwrap();
    f.platform
        .install_file_manager_entries(Path::new("/opt/carrova"))
        .unwrap();

    let status = f.platform.remove_file_manager_entries().unwrap();
    assert!(status.supported && !status.installed);
    for rel in ENTRY_FILES {
        assert!(!f.data(rel).exists(), "{rel}");
    }
    assert_eq!(fs::read_to_string(&neighbour).unwrap(), "mine");
    assert_eq!(f.read(&f.uca()), USER_UCA);
}

#[test]
fn remove_with_nothing_installed_is_a_no_op() {
    let f = Fixture::new();
    let status = f.platform.remove_file_manager_entries().unwrap();
    assert!(status.supported && !status.installed);
    assert!(!f.uca().exists());
}

#[test]
fn status_reports_supported_on_linux() {
    let f = Fixture::new();
    assert_eq!(
        f.platform.file_manager_status().unwrap(),
        FileManagerStatus {
            supported: true,
            installed: false,
            skipped: vec![],
        }
    );
    // Any one of our files counts as installed.
    let script = f.data("nautilus/scripts/Send with Carrova");
    fs::create_dir_all(script.parent().unwrap()).unwrap();
    fs::write(&script, "").unwrap();
    assert!(f.platform.file_manager_status().unwrap().installed);
}

#[test]
fn status_counts_thunar_action_alone_as_installed() {
    let f = Fixture::new();
    fs::create_dir_all(f.uca().parent().unwrap()).unwrap();
    fs::write(
        f.uca(),
        "<actions>\n<action><unique-id>carrova-send-with</unique-id></action>\n</actions>\n",
    )
    .unwrap();
    assert!(f.platform.file_manager_status().unwrap().installed);
}

// ---- Thunar ----

#[test]
fn thunar_merge_keeps_user_actions_byte_for_byte() {
    let f = Fixture::new();
    fs::create_dir_all(f.uca().parent().unwrap()).unwrap();
    fs::write(f.uca(), USER_UCA).unwrap();
    fs::set_permissions(f.uca(), fs::Permissions::from_mode(0o600)).unwrap();
    f.platform
        .install_file_manager_entries(Path::new("/opt/carrova"))
        .unwrap();

    let merged = f.read(&f.uca());
    let user_part = USER_UCA.strip_suffix("</actions>\n").unwrap();
    assert!(merged.starts_with(user_part));
    assert!(merged.ends_with("</action>\n</actions>\n"));
    assert_eq!(f.mode(&f.uca()), 0o600);
}

#[test]
fn thunar_install_twice_keeps_one_action() {
    let f = Fixture::new();
    fs::create_dir_all(f.uca().parent().unwrap()).unwrap();
    fs::write(f.uca(), USER_UCA).unwrap();
    for _ in 0..2 {
        f.platform
            .install_file_manager_entries(Path::new("/opt/carrova"))
            .unwrap();
    }
    let merged = f.read(&f.uca());
    assert_eq!(merged.matches("carrova-send-with").count(), 1);
    assert_eq!(merged.matches("Open Terminal Here").count(), 1);
}

#[test]
fn thunar_malformed_file_is_skipped_not_overwritten() {
    for broken in [
        "<actions>\n<action>\n",
        "not xml at all",
        "<actions>\n<unique-id>carrova-send-with</unique-id>\n</actions>\n",
        // Our action never closes before a user's action opens: cutting to
        // the next `</action>` would take the user's action with it.
        "<actions>\n<action><unique-id>carrova-send-with</unique-id>\n\
         <action><name>Mine</name></action>\n</actions>\n",
    ] {
        let f = Fixture::new();
        fs::create_dir_all(f.uca().parent().unwrap()).unwrap();
        fs::write(f.uca(), broken).unwrap();

        let status = f
            .platform
            .install_file_manager_entries(Path::new("/opt/carrova"))
            .unwrap();
        assert_eq!(
            status.skipped,
            vec!["Thunar: uca.xml is not valid; left unchanged".to_owned()]
        );
        assert!(status.installed);
        assert_eq!(f.read(&f.uca()), broken);
        let removed = f.platform.remove_file_manager_entries().unwrap();
        assert_eq!(removed.skipped.len(), 1);
        assert_eq!(f.read(&f.uca()), broken);
    }
}

#[test]
fn thunar_non_utf8_file_is_skipped_not_overwritten() {
    let f = Fixture::new();
    fs::create_dir_all(f.uca().parent().unwrap()).unwrap();
    fs::write(f.uca(), b"<actions>\xff</actions>").unwrap();
    let status = f
        .platform
        .install_file_manager_entries(Path::new("/opt/carrova"))
        .unwrap();
    assert_eq!(status.skipped.len(), 1);
    assert_eq!(fs::read(f.uca()).unwrap(), b"<actions>\xff</actions>");
}

#[test]
fn thunar_linked_file_is_skipped_not_replaced() {
    let f = Fixture::new();
    let real = f.tmp.path().join("dotfiles-uca.xml");
    fs::write(&real, USER_UCA).unwrap();
    fs::create_dir_all(f.uca().parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(&real, f.uca()).unwrap();

    let status = f
        .platform
        .install_file_manager_entries(Path::new("/opt/carrova"))
        .unwrap();
    assert_eq!(
        status.skipped,
        vec!["Thunar: uca.xml is a link; left unchanged".to_owned()]
    );
    assert!(fs::symlink_metadata(f.uca()).unwrap().is_symlink());
    assert_eq!(f.read(&real), USER_UCA);
}

#[test]
fn thunar_missing_file_is_created() {
    let f = Fixture::new();
    f.platform
        .install_file_manager_entries(Path::new("/opt/carrova"))
        .unwrap();
    let text = f.read(&f.uca());
    assert!(text.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<actions>\n<action>\n"));
    assert!(text.ends_with("</action>\n</actions>\n"));
    assert_eq!(
        text.matches("<unique-id>carrova-send-with</unique-id>")
            .count(),
        1
    );
}

#[test]
fn thunar_edits_find_our_action_by_id_only() {
    let added = thunar::with_action(
        USER_UCA,
        "<action><unique-id>carrova-send-with</unique-id></action>\n",
    )
    .unwrap();
    assert_eq!(thunar::without_action(&added).unwrap(), USER_UCA);
    assert!(thunar::has_action(&added));
    assert!(!thunar::has_action(USER_UCA));
}
