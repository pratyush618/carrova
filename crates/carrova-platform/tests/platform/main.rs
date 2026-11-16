//! Trait defaults: an adapter that overrides only the required methods.

use std::path::{Path, PathBuf};
use std::time::Duration;

use carrova_platform::phone::{AudioSide, PhonePerm};
use carrova_platform::{
    ClipContent, ClipKind, OsKind, Platform, PlatformError, ProcessInfo, SemanticRoot,
    SnapshotCapability,
};

#[derive(Debug)]
struct Minimal;

impl Platform for Minimal {
    fn os(&self) -> OsKind {
        OsKind::Linux
    }
    fn resolve_root(&self, root: SemanticRoot) -> Result<PathBuf, PlatformError> {
        {
            let _ = root;
            Err(PlatformError::Unsupported("resolve_root"))
        }
    }
    fn find_binary(&self, _: &str) -> Result<Option<PathBuf>, PlatformError> {
        Ok(None)
    }
    fn metadata(&self, _: &Path) -> Result<carrova_platform::FileMeta, PlatformError> {
        Err(PlatformError::Unsupported("metadata"))
    }
    fn running_processes(&self) -> Result<Vec<ProcessInfo>, PlatformError> {
        Ok(Vec::new())
    }
    fn free_space(&self, _: &Path) -> Result<u64, PlatformError> {
        Ok(0)
    }
    fn snapshot_capability(&self, _: &Path) -> Result<SnapshotCapability, PlatformError> {
        Ok(SnapshotCapability::None)
    }
}

#[test]
fn default_methods_unsupported() {
    let p = Minimal;
    let clip = ClipContent {
        kind: ClipKind::Text,
        bytes: Vec::new(),
        sensitive: false,
    };
    let path = Path::new("/x");
    assert!(matches!(
        p.clipboard_read(),
        Err(PlatformError::Unsupported(_))
    ));
    assert!(matches!(
        p.clipboard_write(&clip),
        Err(PlatformError::Unsupported(_))
    ));
    assert!(matches!(
        p.watch_clipboard(Box::new(|| {})),
        Err(PlatformError::Unsupported(_))
    ));
    assert!(matches!(
        p.downloads_dir(),
        Err(PlatformError::Unsupported(_))
    ));
    assert!(matches!(
        p.open_url("https://example.org"),
        Err(PlatformError::Unsupported(_))
    ));
    assert!(matches!(
        p.open_path(path),
        Err(PlatformError::Unsupported(_))
    ));
    assert!(matches!(p.ring(5), Err(PlatformError::Unsupported(_))));
    assert!(matches!(p.stop_ring(), Err(PlatformError::Unsupported(_))));
    assert!(matches!(
        p.bind_local_socket(path),
        Err(PlatformError::Unsupported(_))
    ));
    assert!(matches!(
        p.connect_local_socket(path),
        Err(PlatformError::Unsupported(_))
    ));
}

#[test]
fn defaults_that_succeed() {
    let p = Minimal;
    assert!(p.multicast_lock().is_ok());
    assert_eq!(p.link_keepalive(), Duration::from_secs(15));
}

/// Asserts `$call` fails with `Unsupported($name)`.
macro_rules! unsupported {
    ($call:expr, $name:literal) => {
        assert!(
            matches!($call, Err(PlatformError::Unsupported($name))),
            "{} is not Unsupported({:?})",
            stringify!($call),
            $name
        );
    };
}

#[test]
fn phase2_defaults_unsupported() {
    let p = Minimal;
    let perm = PhonePerm::Sms;
    unsupported!(p.watch_phone_events(Box::new(|_| {})), "watch_phone_events");
    unsupported!(p.active_notes(), "active_notes");
    unsupported!(p.note_reply("k", "hi"), "note_reply");
    unsupported!(p.note_dismiss("k"), "note_dismiss");
    unsupported!(p.note_open("k"), "note_open");
    unsupported!(p.sms_threads(None, 10, None), "sms_threads");
    unsupported!(p.sms_thread(1, None, 10), "sms_thread");
    unsupported!(p.sms_send("1", "hi"), "sms_send");
    unsupported!(p.call_answer(), "call_answer");
    unsupported!(p.call_decline(), "call_decline");
    unsupported!(p.call_end(), "call_end");
    unsupported!(p.call_mute(true), "call_mute");
    unsupported!(p.call_dial("1"), "call_dial");
    unsupported!(p.call_speaker(true), "call_speaker");
    unsupported!(p.call_history(None, 10), "call_history");
    unsupported!(p.phone_perm(perm), "phone_perm");
    unsupported!(p.open_perm_settings(perm), "open_perm_settings");
    unsupported!(p.call_audio_status(), "call_audio_status");
    unsupported!(p.install_call_audio(), "install_call_audio");
    unsupported!(p.set_call_audio_config(true), "set_call_audio_config");
    unsupported!(p.bt_pair(Box::new(|_| {})), "bt_pair");
    unsupported!(p.call_tones("12#"), "call_tones");
    unsupported!(p.call_audio_to(AudioSide::Laptop), "call_audio_to");
}
