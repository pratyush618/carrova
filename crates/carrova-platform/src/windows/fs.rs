//! Safe filesystem primitives (Windows): the same `pub(crate)` API as
//! `fsops`, so the fake platform works through `crate::sys` on every OS.
//!
//! "No-follow" means the final component is opened with
//! `FILE_FLAG_OPEN_REPARSE_POINT` (so a symlink or junction is opened as
//! itself, never its target) and the opened handle is then rejected if it
//! is a reparse point or not a regular file. Windows has no mode bits
//! ([`crate::PermissionModel::Acl`]): `mode` arguments are validated and
//! otherwise ignored; per-user privacy comes from the profile ACLs.

use std::ffi::OsStr;
use std::fs::{File, Metadata, OpenOptions};
use std::io::{self, Seek, SeekFrom};
use std::os::windows::fs::{FileExt, MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use crate::path::normalize_abs;
use crate::{Allocation, DirEntry, FileKind, FileLock, FsId, PlatformError};

/// Open a reparse point (symlink, junction) itself, not its target.
const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x10;
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
/// `ERROR_PRIVILEGE_NOT_HELD`: symlink creation without Developer Mode.
const ERROR_PRIVILEGE_NOT_HELD: i32 = 1314;
/// Permission bits a caller may pass; anything else is refused, as on Unix.
const PERM_MASK: u32 = 0o777;

fn io_err(path: &Path) -> impl FnOnce(io::Error) -> PlatformError + '_ {
    move |e| PlatformError::io(path, e)
}

fn invalid(path: &Path, msg: &'static str) -> PlatformError {
    PlatformError::io(path, io::Error::new(io::ErrorKind::InvalidInput, msg))
}

fn check_mode(path: &Path, mode: u32) -> Result<(), PlatformError> {
    if mode & !PERM_MASK != 0 {
        return Err(invalid(path, "mode has bits outside 0o777"));
    }
    Ok(())
}

fn is_reparse(meta: &Metadata) -> bool {
    meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

/// Open `path` with `opts` without following a final reparse point, and
/// keep the handle only if it is a plain regular file.
fn open_regular(path: &Path, opts: &mut OpenOptions) -> Result<File, PlatformError> {
    let file = opts
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
        .map_err(io_err(path))?;
    let meta = file.metadata().map_err(io_err(path))?;
    if is_reparse(&meta) {
        return Err(invalid(path, "is a symlink or junction"));
    }
    if !meta.is_file() {
        return Err(invalid(path, "not a regular file"));
    }
    Ok(file)
}

/// See [`crate::Platform::open_read_nofollow`].
pub(crate) fn open_read_nofollow(path: &Path) -> Result<File, PlatformError> {
    open_regular(path, OpenOptions::new().read(true))
}

/// See [`crate::Platform::open_append_nofollow`].
///
/// Windows append-only handles (no `FILE_WRITE_DATA`) cannot be truncated,
/// so this opens for writing and moves the cursor to the end instead:
/// writes go to the cursor, and a caller that shrinks the file with
/// `set_len` must seek to the end again before writing.
pub(crate) fn open_append_nofollow(path: &Path) -> Result<File, PlatformError> {
    let mut file = open_regular(path, OpenOptions::new().write(true))?;
    file.seek(SeekFrom::End(0)).map_err(io_err(path))?;
    Ok(file)
}

/// See [`crate::Platform::open_rw_nofollow`].
pub(crate) fn open_rw_nofollow(path: &Path) -> Result<File, PlatformError> {
    open_regular(path, OpenOptions::new().read(true).write(true))
}

/// See [`crate::Platform::create_new_file`]. `CREATE_NEW` fails on any
/// existing name, a dangling link included. `mode` is checked but not
/// applied (ACL model): the new file inherits the parent's ACL.
pub(crate) fn create_new_file(path: &Path, mode: Option<u32>) -> Result<File, PlatformError> {
    if let Some(m) = mode {
        check_mode(path, m)?;
    }
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
        .map_err(io_err(path))
}

/// See [`crate::Platform::create_dir`]. `mode` is checked, not applied.
pub(crate) fn create_dir(path: &Path, mode: Option<u32>) -> Result<(), PlatformError> {
    if let Some(m) = mode {
        check_mode(path, m)?;
    }
    std::fs::create_dir(path).map_err(io_err(path))
}

/// See [`crate::Platform::set_mode`]: a no-op on a regular file or
/// directory (ACL model), still refusing links and special files.
pub(crate) fn set_mode(path: &Path, mode: u32) -> Result<(), PlatformError> {
    check_mode(path, mode)?;
    let meta = std::fs::symlink_metadata(path).map_err(io_err(path))?;
    if is_reparse(&meta) {
        return Err(invalid(path, "is a symlink or junction"));
    }
    if !(meta.is_file() || meta.is_dir()) {
        return Err(invalid(path, "not a regular file or directory"));
    }
    Ok(())
}

/// See [`crate::Platform::create_symlink`]. Always a *file* symlink.
///
/// shortcut: a target that is a directory gets a file symlink (unusable as
/// a dir), since the target is never resolved; add a kind hint to the trait
/// when a recipe needs directory links on Windows.
pub(crate) fn create_symlink(target: &str, link: &Path) -> Result<(), PlatformError> {
    std::os::windows::fs::symlink_file(target, link).map_err(|e| {
        if e.raw_os_error() == Some(ERROR_PRIVILEGE_NOT_HELD) {
            let msg = "creating symlinks needs Windows Developer Mode or administrator rights";
            PlatformError::io(link, io::Error::new(io::ErrorKind::PermissionDenied, msg))
        } else {
            PlatformError::io(link, e)
        }
    })
}

/// See [`crate::Platform::read_link`].
pub(crate) fn read_link(path: &Path) -> Result<PathBuf, PlatformError> {
    std::fs::read_link(path).map_err(io_err(path))
}

/// See [`crate::Platform::rename_noreplace`].
///
/// - Regular file: `hard_link` (`CreateHardLinkW` fails if `to` exists and
///   links a symlink itself, never its target), then remove `from`.
/// - Anything else (directory, link): `std::fs::rename`. Per the
///   `MoveFileExW` and `FILE_RENAME_INFORMATION` docs, replacing a target
///   that *is a directory* always fails, but a file at `to` could be
///   replaced, so its absence is checked first.
///
/// shortcut: for non-files the check-then-rename leaves a race in which a
/// file created at `to` meanwhile is replaced; acceptable inside
/// Carrova-owned staging dirs, revisit if this runs on shared directories.
/// shortcut: hard links need NTFS/ReFS; FAT volumes fail here, add a
/// fallback if staging on FAT is ever supported.
pub(crate) fn rename_noreplace(from: &Path, to: &Path) -> Result<(), PlatformError> {
    let meta = std::fs::symlink_metadata(from).map_err(io_err(from))?;
    let at = |e: io::Error| {
        let at = if e.kind() == io::ErrorKind::AlreadyExists {
            to
        } else {
            from
        };
        PlatformError::io(at, e)
    };
    if meta.is_file() {
        std::fs::hard_link(from, to).map_err(at)?;
        return std::fs::remove_file(from).map_err(io_err(from));
    }
    if std::fs::symlink_metadata(to).is_ok() {
        return Err(at(io::ErrorKind::AlreadyExists.into()));
    }
    std::fs::rename(from, to).map_err(at)
}

/// See [`crate::Platform::rename_replace`]: `MoveFileExW` with
/// `MOVEFILE_REPLACE_EXISTING`, which renames a link as itself.
pub(crate) fn rename_replace(from: &Path, to: &Path) -> Result<(), PlatformError> {
    std::fs::rename(from, to).map_err(io_err(from))
}

/// See [`crate::Platform::sync_dir`]: a no-op, because NTFS journals
/// directory entries itself and std cannot flush a directory handle.
pub(crate) fn sync_dir(_path: &Path) -> Result<(), PlatformError> {
    Ok(())
}

/// See [`crate::Platform::sync_fs`]: unsupported (flushing a volume needs
/// administrator rights); callers fall back to per-file syncs.
pub(crate) fn sync_fs(_path: &Path) -> Result<(), PlatformError> {
    Err(PlatformError::Unsupported("sync_fs"))
}

/// See [`crate::Platform::remove_file`]. A directory symlink or junction
/// is removed as a link (`RemoveDirectoryW` on the reparse point never
/// touches its target); a real directory is refused.
pub(crate) fn remove_file(path: &Path) -> Result<(), PlatformError> {
    let meta = std::fs::symlink_metadata(path).map_err(io_err(path))?;
    let dir_link = is_reparse(&meta) && meta.file_attributes() & FILE_ATTRIBUTE_DIRECTORY != 0;
    if dir_link {
        std::fs::remove_dir(path).map_err(io_err(path))
    } else {
        std::fs::remove_file(path).map_err(io_err(path))
    }
}

/// See [`crate::Platform::read_dir`]. Entry kinds come from the directory
/// listing itself (lstat semantics: links are reported as links).
///
/// shortcut: `path` is checked to be a real directory and then listed by
/// path, so a swap to a junction in between is not caught; std cannot list
/// a pinned handle without unsafe.
pub(crate) fn read_dir(path: &Path) -> Result<Vec<DirEntry>, PlatformError> {
    let meta = std::fs::symlink_metadata(path).map_err(io_err(path))?;
    if is_reparse(&meta) || !meta.is_dir() {
        return Err(invalid(path, "not a directory (or a link to one)"));
    }
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(path).map_err(io_err(path))? {
        let entry = entry.map_err(io_err(path))?;
        let ft = entry.file_type().map_err(io_err(path))?;
        let kind = if ft.is_symlink() {
            FileKind::Symlink
        } else if ft.is_dir() {
            FileKind::Dir
        } else if ft.is_file() {
            FileKind::File
        } else {
            FileKind::Other
        };
        entries.push(DirEntry {
            name: entry.file_name(),
            kind,
        });
    }
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(entries)
}

/// See [`crate::Platform::remove_dir_all_confined`].
///
/// Every component between `within` and `path` must be a real directory
/// (not a link). A link at `path` is removed as a link; a directory is
/// removed with `std::fs::remove_dir_all`, which on Windows works on
/// handles and removes links inside the tree without traversing them.
///
/// shortcut: the ancestor checks are path-based, so swapping an ancestor
/// for a junction between check and delete is not caught; std offers no
/// handle-relative open without unsafe.
pub(crate) fn remove_dir_all_confined(path: &Path, within: &Path) -> Result<(), PlatformError> {
    let not_inside = || {
        invalid(
            path,
            "path is not strictly inside the confinement directory",
        )
    };
    let norm_within = normalize_abs(within).ok_or_else(|| invalid(within, "not absolute"))?;
    let norm_path = normalize_abs(path).ok_or_else(not_inside)?;
    let rel = norm_path
        .strip_prefix(&norm_within)
        .map_err(|_| not_inside())?;
    let mut comps: Vec<&OsStr> = rel.iter().collect();
    if comps.pop().is_none() {
        return Err(not_inside());
    }
    let mut dir = norm_within.clone();
    for c in comps {
        dir.push(c);
        let meta = std::fs::symlink_metadata(&dir).map_err(io_err(path))?;
        if is_reparse(&meta) || !meta.is_dir() {
            return Err(invalid(path, "an ancestor is a link or not a directory"));
        }
    }
    let meta = std::fs::symlink_metadata(&norm_path).map_err(io_err(path))?;
    if meta.is_dir() && !is_reparse(&meta) {
        std::fs::remove_dir_all(&norm_path).map_err(io_err(path))
    } else {
        remove_file(&norm_path)
    }
}

/// See [`crate::Platform::fs_id`]: the volume serial number of the
/// volume holding `path` (following links).
pub(crate) fn fs_id(path: &Path) -> Result<FsId, PlatformError> {
    let handle = winapi_util::Handle::from_path_any(path).map_err(io_err(path))?;
    let info = winapi_util::file::information(&handle).map_err(io_err(path))?;
    Ok(FsId::new(info.volume_serial_number()))
}

/// See [`crate::Platform::write_all_at`]. Unlike `pwrite`, `seek_write`
/// moves the file cursor to the end of the written range.
pub(crate) fn write_all_at(file: &File, mut buf: &[u8], mut offset: u64) -> io::Result<()> {
    while !buf.is_empty() {
        match file.seek_write(buf, offset) {
            Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
            Ok(n) => {
                buf = &buf[n..];
                offset += n as u64;
            }
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

/// See [`crate::Platform::preallocate`]: `set_len` only. NTFS allocates on
/// extend, but std gives no guarantee, so this reports the weaker
/// [`Allocation::Sparse`].
pub(crate) fn preallocate(file: &File, len: u64) -> io::Result<Allocation> {
    file.set_len(len)?;
    Ok(Allocation::Sparse)
}

/// See [`crate::Platform::lock_exclusive`]: std `File::lock`
/// (`LockFileEx`). Unlike `flock` the lock is mandatory for the file's
/// bytes, which is harmless for Carrova's dedicated, empty lock files.
pub(crate) fn lock_exclusive(path: &Path) -> Result<FileLock, PlatformError> {
    let file = open_regular(path, OpenOptions::new().read(true).write(true).create(true))?;
    file.lock().map_err(io_err(path))?;
    Ok(FileLock::held(file))
}

/// Minimal tool environment: `USERPROFILE` = `home`, `PATH` (if known),
/// and the system variables any Windows program needs to start
/// (`SystemRoot`, `PATHEXT`, `TEMP`), copied from this process.
pub(crate) fn tool_command(bin: &Path, home: &Path, path: Option<&OsStr>) -> std::process::Command {
    use std::os::windows::process::CommandExt;
    // Tools are non-interactive; without this a console tool run from the
    // GUI app flashes a console window.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let mut cmd = std::process::Command::new(bin);
    cmd.creation_flags(CREATE_NO_WINDOW)
        .env_clear()
        .env("USERPROFILE", home);
    if let Some(p) = path {
        cmd.env("PATH", p);
    }
    for var in ["SystemRoot", "PATHEXT", "TEMP"] {
        if let Some(v) = std::env::var_os(var) {
            cmd.env(var, v);
        }
    }
    cmd
}

/// shortcut: no process tree on Windows (a job object needs `unsafe` or a
/// new crate), so [`kill_tool`] ends only the tool itself; add a job object
/// once Windows plugins run tools that spawn helpers.
pub(crate) fn isolate_tool(_cmd: &mut std::process::Command) {}

/// Kill the tool (not its children, see [`isolate_tool`]).
pub(crate) fn kill_tool(child: &mut std::process::Child) -> std::io::Result<()> {
    child.kill()
}
