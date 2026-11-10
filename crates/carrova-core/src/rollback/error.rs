//! [`RollbackStepError`]: why one journalled step could not be undone.

use carrova_platform::PlatformError;

use crate::error::CoreError;

/// Why a step was left for manual attention. Shown to the user as text in
/// [`super::RollbackReport::failed`]; never carries file contents.
#[derive(Debug, thiserror::Error)]
pub(super) enum RollbackStepError {
    #[error("temp path is no longer a regular file")]
    TempNotAFile,
    #[error("changed since apply; left in place")]
    Changed,
    #[error("no longer a regular file; left in place")]
    NotAFile,
    #[error("symlink target changed; left in place")]
    LinkTargetChanged,
    #[error("no longer a symlink; left in place")]
    NotASymlink,
    #[error("directory not empty; left in place")]
    DirNotEmpty,
    #[error("no longer a directory; left in place")]
    NotADir,
    #[error("holds entries rollback left in place")]
    HoldsLeftovers,
    #[error("rollback trash is not a directory; left in place")]
    TrashNotADir,
    #[error("cannot quarantine (rollback store is on another filesystem); left in place")]
    TrashOnOtherFilesystem,
    #[error("no free quarantine name; left in place")]
    NoTrashName,
    #[error("original location is occupied; backup kept")]
    Occupied,
    #[error(transparent)]
    Core(#[from] CoreError),
    #[error(transparent)]
    Platform(#[from] PlatformError),
}

/// Result of undoing one step.
pub(super) type StepResult<T> = Result<T, RollbackStepError>;
