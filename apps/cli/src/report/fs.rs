//! Report files through the platform layer: owner-only (`0600`), never
//! written through a symlink.

use std::fs::File;
use std::io;
use std::path::Path;
use std::sync::Arc;

use carrova_platform::{Platform, PlatformError};
use carrova_report::ReportFs;

/// Permission bits of every report and the consent file.
const PRIVATE_MODE: u32 = 0o600;

/// [`ReportFs`] over a [`Platform`].
#[derive(Debug)]
pub struct PlatformReportFs(pub Arc<dyn Platform>);

impl ReportFs for PlatformReportFs {
    fn create_private(&self, path: &Path) -> io::Result<File> {
        self.0
            .create_new_file(path, Some(PRIVATE_MODE))
            .map_err(into_io)
    }

    fn replace(&self, from: &Path, to: &Path) -> io::Result<()> {
        self.0.rename_replace(from, to).map_err(into_io)
    }
}

/// The underlying I/O error (the store adds the path itself).
fn into_io(err: PlatformError) -> io::Error {
    match err {
        PlatformError::Io { source, .. } => source,
        other => io::Error::other(other),
    }
}
