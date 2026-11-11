use crate::linux::{LinuxEnv, LinuxPlatform};
use crate::{Platform, PlatformError};
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

const LAUNCHER: &str = "/opt/Carrova App/carrova";

fn platform(tmp: &TempDir) -> LinuxPlatform {
    LinuxPlatform::new(LinuxEnv {
        home: Some(tmp.path().into()),
        xdg_config_home: Some(tmp.path().join("config").into()),
        ..LinuxEnv::default()
    })
}

fn entry(tmp: &TempDir) -> PathBuf {
    tmp.path()
        .join("config/autostart/org.byteveda.Carrova.desktop")
}

#[test]
fn autostart_entry_written_and_removed() {
    let tmp = TempDir::new().unwrap();
    let p = platform(&tmp);
    assert!(!p.autostart_status().unwrap());

    assert!(p.set_autostart(Path::new(LAUNCHER), true).unwrap());
    let text = fs::read_to_string(entry(&tmp)).unwrap();
    assert!(text.starts_with("[Desktop Entry]\n"), "{text}");
    assert!(
        text.contains("\nExec=\"/opt/Carrova App/carrova\" --background\n"),
        "{text}"
    );
    assert!(p.autostart_status().unwrap());

    assert!(!p.set_autostart(Path::new(LAUNCHER), false).unwrap());
    assert!(!entry(&tmp).exists());
    // Already off: nothing to remove, still off.
    assert!(!p.set_autostart(Path::new(LAUNCHER), false).unwrap());
}

#[test]
fn launcher_with_percent_refused() {
    let tmp = TempDir::new().unwrap();
    let p = platform(&tmp);
    let refused = p.set_autostart(Path::new("/opt/100%/carrova"), true);
    assert!(
        matches!(refused, Err(PlatformError::Unsupported(_))),
        "{refused:?}"
    );
    assert!(!entry(&tmp).exists());
}
