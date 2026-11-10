//! Semantic roots, the state dir and the GnuPG home from Windows known
//! folders, with the same confinement as Linux: never a drive root.

use std::path::{Path, PathBuf};

use known_folders::{KnownFolder, get_known_folder_path};

use crate::{PlatformError, SemanticRoot};

/// Known folder backing each semantic root. `cache` shares LocalAppData
/// with `user_data`: Windows apps keep caches under their own app dir.
fn known_folder(root: SemanticRoot) -> KnownFolder {
    match root {
        SemanticRoot::UserConfig => KnownFolder::RoamingAppData,
        SemanticRoot::UserData | SemanticRoot::Cache => KnownFolder::LocalAppData,
        SemanticRoot::Home => KnownFolder::Profile,
        SemanticRoot::Documents => KnownFolder::Documents,
    }
}

/// Confined path of `root` for the current user.
pub(super) fn root_path(root: SemanticRoot) -> Result<PathBuf, PlatformError> {
    let path = get_known_folder_path(known_folder(root)).ok_or_else(|| {
        PlatformError::RootUnavailable {
            root,
            reason: "known folder is not available".into(),
        }
    })?;
    confine(root, &path)
}

/// `%LOCALAPPDATA%\Carrova\state`. Not created by this call.
pub(super) fn state_dir() -> Result<PathBuf, PlatformError> {
    root_path(SemanticRoot::UserData)
        .map(|local| local.join("Carrova").join("state"))
        .map_err(|e| PlatformError::StateDirUnavailable(e.to_string()))
}

/// `%APPDATA%\gnupg`, Gpg4win's default home (an inherited `GNUPGHOME`
/// is ignored, as on Linux).
pub(super) fn gnupg_home() -> Result<PathBuf, PlatformError> {
    Ok(root_path(SemanticRoot::UserConfig)?.join("gnupg"))
}

/// Lexically normalise an absolute root candidate and refuse a drive or
/// share root (`C:\`, `\\server\share\`): such a root would make "inside a
/// semantic root" mean "anywhere on the volume", voiding confinement.
pub(super) fn confine(root: SemanticRoot, path: &Path) -> Result<PathBuf, PlatformError> {
    crate::path::normalize_abs(path)
        .filter(|p| p.parent().is_some())
        .ok_or_else(|| PlatformError::RootUnavailable {
            root,
            reason: "path is not absolute, or is a drive root".into(),
        })
}
