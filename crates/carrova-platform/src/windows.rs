//! Stub adapter: compiles, returns `Unsupported` (Phase 5 implements it).

use std::path::{Path, PathBuf};

use crate::{
    FileMeta, OsKind, Platform, PlatformError, ProcessInfo, SemanticRoot, SnapshotCapability,
};

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct WindowsPlatform;

impl Platform for WindowsPlatform {
    fn os(&self) -> OsKind {
        OsKind::Windows
    }

    fn resolve_root(&self, _root: SemanticRoot) -> Result<PathBuf, PlatformError> {
        Err(PlatformError::Unsupported("resolve_root"))
    }

    fn find_binary(&self, _name: &str) -> Result<Option<PathBuf>, PlatformError> {
        Err(PlatformError::Unsupported("find_binary"))
    }

    fn metadata(&self, _path: &Path) -> Result<FileMeta, PlatformError> {
        Err(PlatformError::Unsupported("metadata"))
    }

    fn running_processes(&self) -> Result<Vec<ProcessInfo>, PlatformError> {
        Err(PlatformError::Unsupported("running_processes"))
    }

    fn free_space(&self, _path: &Path) -> Result<u64, PlatformError> {
        Err(PlatformError::Unsupported("free_space"))
    }

    fn snapshot_capability(&self, _path: &Path) -> Result<SnapshotCapability, PlatformError> {
        Err(PlatformError::Unsupported("snapshot_capability"))
    }
}
