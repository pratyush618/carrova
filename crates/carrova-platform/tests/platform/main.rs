//! Trait defaults: an adapter that overrides only the required methods.

use std::path::{Path, PathBuf};
use std::time::Duration;

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
