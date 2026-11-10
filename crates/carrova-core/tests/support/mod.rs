//! Helpers shared by the carrova-core integration test crates: recipe
//! loading, fixture files with exact modes, and [`FakePlatform`] fault hooks.
//!
//! [`FakePlatform`]: carrova_platform::fake::FakePlatform

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use carrova_platform::fake::{OpenHook, RenameHook};
use carrova_platform::{Platform, PlatformError};
use carrova_recipe::Recipe;

/// Load one recipe fixture (`yaml`) as `app/recipe.yaml`.
pub fn recipes(yaml: &str) -> Vec<Recipe> {
    vec![carrova_recipe::load_str(yaml, Path::new("app/recipe.yaml")).unwrap()]
}

/// [`put_at`] `home/rel`.
pub fn put(p: &dyn Platform, home: &Path, rel: &str, bytes: &[u8], mode: u32) {
    put_at(p, &home.join(rel), bytes, mode);
}

/// Write `bytes` to a fresh file at `path` (parents created, an existing
/// file replaced) with exactly `mode`, whatever the umask.
pub fn put_at(p: &dyn Platform, path: &Path, bytes: &[u8], mode: u32) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    if path.exists() {
        std::fs::remove_file(path).unwrap();
    }
    let mut f = p.create_new_file(path, Some(mode)).unwrap();
    f.write_all(bytes).unwrap();
}

/// Create the directory `home/rel` (and parents) and give it exactly `mode`.
pub fn mkdir(p: &dyn Platform, home: &Path, rel: &str, mode: u32) {
    let path = home.join(rel);
    std::fs::create_dir_all(&path).unwrap();
    p.set_mode(&path, mode).unwrap();
}

/// A platform I/O error of `kind` on `path`.
pub fn io_err(path: &Path, kind: std::io::ErrorKind) -> PlatformError {
    PlatformError::Io {
        path: path.to_path_buf(),
        source: std::io::Error::from(kind),
    }
}

/// Renames out of `dir` fail with `CrossesDevices` (staging on another
/// filesystem).
pub fn cross_device_from(dir: PathBuf) -> RenameHook {
    Arc::new(move |from, _| {
        if from.starts_with(&dir) {
            return Err(io_err(from, std::io::ErrorKind::CrossesDevices));
        }
        Ok(())
    })
}

/// The next rename onto `to` fails with `PermissionDenied` (once).
pub fn fail_rename_once(to: PathBuf) -> RenameHook {
    let pending = Mutex::new(Some(to));
    Arc::new(move |_, to| {
        let mut pending = pending.lock().unwrap();
        if pending.as_deref() == Some(to) {
            *pending = None;
            return Err(io_err(to, std::io::ErrorKind::PermissionDenied));
        }
        Ok(())
    })
}

/// Right after `opened` is opened, `replacement` is renamed over it (a swap
/// racing the open; once).
pub fn swap_after_open(opened: PathBuf, replacement: PathBuf) -> OpenHook {
    let pending = Mutex::new(Some((opened, replacement)));
    Arc::new(move |path| {
        let mut pending = pending.lock().unwrap();
        if pending.as_ref().is_some_and(|(opened, _)| opened == path) {
            let (opened, replacement) = pending.take().unwrap();
            std::fs::rename(replacement, opened).unwrap();
        }
    })
}
