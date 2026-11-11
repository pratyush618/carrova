use super::*;
use crate::macos::{MacEnv, MacPlatform};
use crate::{FileManagerStatus, Platform, PlatformError};
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

const APP_EXE: &str = "/Applications/Carrova.app/Contents/MacOS/carrova-desktop";
const DEV_EXE: &str = "/Users/u/carrova/target/debug/carrova-desktop";

fn unsupported(launcher: &str) -> bool {
    matches!(
        open_command(Path::new(launcher)),
        Err(PlatformError::Unsupported(_))
    )
}

// ---- open command ----

#[test]
fn open_command_targets_the_enclosing_app() {
    assert_eq!(
        open_command(Path::new(APP_EXE)).unwrap(),
        r#"open -n -a '/Applications/Carrova.app' --args --send "$@""#
    );
}

#[test]
fn open_command_picks_the_nearest_app_ancestor() {
    assert_eq!(
        open_command(Path::new(
            "/Volumes/Old.app/Builds/Carrova.app/Contents/MacOS/carrova-desktop"
        ))
        .unwrap(),
        r#"open -n -a '/Volumes/Old.app/Builds/Carrova.app' --args --send "$@""#
    );
}

#[test]
fn open_command_quotes_spaces_and_quotes() {
    assert_eq!(
        open_command(Path::new(
            "/Users/u/My Apps/it's $HOME.app/Contents/MacOS/c"
        ))
        .unwrap(),
        r#"open -n -a '/Users/u/My Apps/it'\''s $HOME.app' --args --send "$@""#
    );
}

#[test]
fn open_command_rejects_a_launcher_outside_an_app() {
    assert!(unsupported(DEV_EXE));
    assert!(unsupported("Carrova.app/Contents/MacOS/c"));
    assert!(unsupported("/Applications/Car\nrova.app/Contents/MacOS/c"));
}

// ---- bundle ----

struct Fixture {
    tmp: TempDir,
    platform: MacPlatform,
}

impl Fixture {
    fn new() -> Self {
        let tmp = TempDir::new().unwrap();
        let platform = MacPlatform::new(MacEnv {
            home: Some(tmp.path().into()),
            ..MacEnv::default()
        });
        Self { tmp, platform }
    }

    fn services(&self) -> PathBuf {
        self.tmp.path().join("Library/Services")
    }

    fn bundle(&self) -> PathBuf {
        self.services().join(BUNDLE)
    }

    fn read(&self, rel: &str) -> String {
        fs::read_to_string(self.bundle().join(rel)).unwrap()
    }

    fn install(&self) -> FileManagerStatus {
        self.platform
            .install_file_manager_entries(Path::new(APP_EXE))
            .unwrap()
    }
}

fn status(installed: bool) -> FileManagerStatus {
    FileManagerStatus {
        supported: true,
        installed,
        skipped: Vec::new(),
    }
}

#[test]
fn install_writes_quick_action_bundle() {
    let f = Fixture::new();
    assert_eq!(f.install(), status(true));

    let info = f.read("Contents/Info.plist");
    for needle in [
        "<key>NSServices</key>",
        "<string>Send with Carrova</string>",
        "<string>runWorkflowAsService</string>",
        "<string>com.apple.finder</string>",
        "<string>public.item</string>",
    ] {
        assert!(info.contains(needle), "Info.plist lacks {needle}");
    }
    let wflow = f.read("Contents/document.wflow");
    for needle in [
        r#"<string>open -n -a '/Applications/Carrova.app' --args --send "$@"</string>"#,
        "<string>com.apple.RunShellScript</string>",
        "<key>inputMethod</key>\n\t\t\t\t\t<integer>1</integer>",
        "<string>com.apple.Automator.servicesMenu</string>",
        "<string>com.apple.Automator.fileSystemObject</string>",
    ] {
        assert!(wflow.contains(needle), "document.wflow lacks {needle}");
    }
}

#[test]
fn install_xml_escapes_the_app_path() {
    let f = Fixture::new();
    f.platform
        .install_file_manager_entries(Path::new("/Apps/A & <B>.app/Contents/MacOS/c"))
        .unwrap();
    assert!(
        f.read("Contents/document.wflow")
            .contains("<string>open -n -a '/Apps/A &amp; &lt;B&gt;.app' --args")
    );
}

#[test]
fn install_outside_an_app_writes_nothing() {
    let f = Fixture::new();
    assert!(matches!(
        f.platform.install_file_manager_entries(Path::new(DEV_EXE)),
        Err(PlatformError::Unsupported(_))
    ));
    assert!(!f.services().exists());
}

#[test]
fn install_refreshes_an_existing_bundle() {
    let f = Fixture::new();
    f.install();
    fs::write(f.bundle().join("Contents/document.wflow"), "stale").unwrap();
    f.install();
    assert!(f.read("Contents/document.wflow").contains("--send"));
}

#[test]
fn remove_deletes_only_the_workflow() {
    let f = Fixture::new();
    let other = f.services().join("Other.workflow/Contents/Info.plist");
    fs::create_dir_all(other.parent().unwrap()).unwrap();
    fs::write(&other, "theirs").unwrap();
    f.install();

    assert_eq!(
        f.platform.remove_file_manager_entries().unwrap(),
        status(false)
    );
    assert!(!f.bundle().exists());
    assert_eq!(fs::read_to_string(&other).unwrap(), "theirs");
}

#[test]
fn status_tracks_install() {
    let f = Fixture::new();
    assert_eq!(f.platform.file_manager_status().unwrap(), status(false));
    f.install();
    assert_eq!(f.platform.file_manager_status().unwrap(), status(true));
    f.platform.remove_file_manager_entries().unwrap();
    assert_eq!(f.platform.file_manager_status().unwrap(), status(false));
    // Removing again is a no-op, not an error.
    assert_eq!(
        f.platform.remove_file_manager_entries().unwrap(),
        status(false)
    );
}
