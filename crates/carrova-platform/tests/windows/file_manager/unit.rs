use winreg::RegKey;
use winreg::enums::{HKEY_CURRENT_USER, KEY_ALL_ACCESS};

use super::*;

const LAUNCHER: &str = r"C:\Program Files\Carrova\Carrova.exe";

/// Entries under a throwaway `HKCU\Software\CarrovaTest\<pid>-<test>\Classes`,
/// deleted on drop.
struct Fixture {
    root: String,
    entries: Entries,
}

impl Fixture {
    fn new(test: &str) -> Self {
        let root = format!(r"Software\CarrovaTest\{}-{test}", std::process::id());
        let (classes, _) = hkcu().create_subkey(format!(r"{root}\Classes")).unwrap();
        Self {
            root,
            entries: Entries::at(classes),
        }
    }

    fn classes(&self) -> RegKey {
        hkcu()
            .open_subkey_with_flags(format!(r"{}\Classes", self.root), KEY_ALL_ACCESS)
            .unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = hkcu().delete_subkey_all(&self.root);
    }
}

fn hkcu() -> RegKey {
    RegKey::predef(HKEY_CURRENT_USER)
}

#[test]
fn install_writes_both_verbs() {
    let fx = Fixture::new("install");
    let status = fx.entries.install(Path::new(LAUNCHER)).unwrap();
    assert!(status.supported && status.installed, "{status:?}");

    for verb in [r"*\shell\Carrova", r"Directory\shell\Carrova"] {
        let key = fx.classes().open_subkey(verb).unwrap();
        assert_eq!(key.get_value::<String, _>("").unwrap(), "Send with Carrova");
        assert_eq!(key.get_value::<String, _>("Icon").unwrap(), LAUNCHER);
        assert_eq!(
            key.get_value::<String, _>("MultiSelectModel").unwrap(),
            "Player"
        );
        let command: String = key.open_subkey("command").unwrap().get_value("").unwrap();
        assert_eq!(command, format!("\"{LAUNCHER}\" --send \"%1\""));
    }
}

#[test]
fn install_refuses_a_launcher_that_breaks_the_command() {
    let fx = Fixture::new("refuse");
    for bad in [
        r"Carrova.exe",
        r#"C:\a"b\Carrova.exe"#,
        "C:\\a\nb\\Carrova.exe",
    ] {
        let err = fx.entries.install(Path::new(bad)).unwrap_err();
        assert!(
            matches!(err, PlatformError::Unsupported(_)),
            "{bad}: {err:?}"
        );
    }
    assert!(!fx.entries.status().unwrap().installed);
}

#[test]
fn remove_deletes_only_our_keys() {
    let fx = Fixture::new("remove");
    let (other, _) = fx.classes().create_subkey(r"*\shell\Other").unwrap();
    other.set_value("", &"Someone else").unwrap();

    fx.entries.install(Path::new(LAUNCHER)).unwrap();
    let status = fx.entries.remove().unwrap();

    assert!(status.supported && !status.installed, "{status:?}");
    for verb in [r"*\shell\Carrova", r"Directory\shell\Carrova"] {
        assert!(fx.classes().open_subkey(verb).is_err(), "{verb}");
    }
    let other: String = fx
        .classes()
        .open_subkey(r"*\shell\Other")
        .unwrap()
        .get_value("")
        .unwrap();
    assert_eq!(other, "Someone else");
    // Removing again finds nothing to delete and still succeeds.
    assert!(!fx.entries.remove().unwrap().installed);
}

#[test]
fn status_tracks_install() {
    let fx = Fixture::new("status");
    let before = fx.entries.status().unwrap();
    assert!(before.supported && !before.installed && before.skipped.is_empty());

    fx.entries.install(Path::new(LAUNCHER)).unwrap();
    assert!(fx.entries.status().unwrap().installed);

    // Either verb left behind still counts as installed.
    fx.entries.remove().unwrap();
    fx.classes()
        .create_subkey(r"Directory\shell\Carrova")
        .unwrap();
    assert!(fx.entries.status().unwrap().installed);
}
