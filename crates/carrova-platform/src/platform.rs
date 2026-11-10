//! The [`Platform`] trait: every OS interaction goes through it.

use std::fs::File;
use std::io;
use std::net::UdpSocket;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{
    Allocation, DirEntry, FileLock, FileMeta, FsId, LanInterface, OsKind, PermissionModel,
    PlatformError, ProcessInfo, SemanticRoot, SnapshotCapability, SocketBuffers,
};

/// OS abstraction used by every service crate.
///
/// Implementations must be cheap to share (`Send + Sync`) and must never follow
/// symlinks unless a method says so.
pub trait Platform: Send + Sync + std::fmt::Debug {
    /// Which OS family this adapter serves.
    fn os(&self) -> OsKind;

    /// How this OS protects files from other local users. Under
    /// [`PermissionModel::Acl`] callers neither store nor enforce mode
    /// bits ([`FileMeta::mode`] is `None`, `mode` arguments are ignored);
    /// per-user privacy comes from the profile directories' ACLs.
    /// Defaults to [`OsKind::permission_model`] of [`Platform::os`].
    fn permission_model(&self) -> PermissionModel {
        self.os().permission_model()
    }

    /// Absolute, lexically normalised path of a semantic root for the current
    /// user. Never the filesystem root (`/`): such a value is reported as
    /// [`PlatformError::RootUnavailable`], since it would void confinement.
    fn resolve_root(&self, root: SemanticRoot) -> Result<PathBuf, PlatformError>;

    /// Locate an executable by name on the user's search path.
    fn find_binary(&self, name: &str) -> Result<Option<PathBuf>, PlatformError>;

    /// Metadata for `path` **without following symlinks** (lstat semantics).
    fn metadata(&self, path: &Path) -> Result<FileMeta, PlatformError>;

    /// Snapshot of currently running processes.
    fn running_processes(&self) -> Result<Vec<ProcessInfo>, PlatformError>;

    /// Bytes available to the current user on the filesystem holding `path`.
    fn free_space(&self, path: &Path) -> Result<u64, PlatformError>;

    /// Whether the filesystem holding `path` supports cheap snapshots.
    fn snapshot_capability(&self, path: &Path) -> Result<SnapshotCapability, PlatformError>;

    // ---- Phase 2: state + safe filesystem writes -------------------------
    //
    // Default bodies return `Unsupported` so stub adapters stay honest.
    // Every method here operates on the final path component **without
    // following symlinks**; callers must still verify ancestors themselves.

    /// Carrova's private per-user state directory (Linux:
    /// `$XDG_STATE_HOME/carrova`, default `~/.local/state/carrova`). Not a
    /// semantic root and never migrated. Not created by this call.
    fn state_dir(&self) -> Result<PathBuf, PlatformError> {
        Err(PlatformError::Unsupported("state_dir"))
    }

    /// Open an existing regular file for reading; fails if `path` is a
    /// symlink (`O_NOFOLLOW`) or not a regular file.
    fn open_read_nofollow(&self, path: &Path) -> Result<File, PlatformError> {
        let _ = path;
        Err(PlatformError::Unsupported("open_read_nofollow"))
    }

    /// Open an existing regular file for appending (`O_WRONLY|O_APPEND`);
    /// fails if `path` is a symlink (`O_NOFOLLOW`), missing (`NotFound`) or
    /// not a regular file (checked with `fstat` on the opened fd, so a
    /// swapped-in link or FIFO is never written through). The returned
    /// file may be truncated with `set_len`.
    fn open_append_nofollow(&self, path: &Path) -> Result<File, PlatformError> {
        let _ = path;
        Err(PlatformError::Unsupported("open_append_nofollow"))
    }

    /// Create a new regular file for writing; fails if anything exists at
    /// `path` (`O_CREAT|O_EXCL|O_NOFOLLOW`). `mode` (`0o777` bits) is applied
    /// exactly, independent of umask; `None` = OS default.
    fn create_new_file(&self, path: &Path, mode: Option<u32>) -> Result<File, PlatformError> {
        let _ = (path, mode);
        Err(PlatformError::Unsupported("create_new_file"))
    }

    /// Create one directory (parent must exist); fails if `path` exists.
    /// `mode` applied exactly, independent of umask.
    fn create_dir(&self, path: &Path, mode: Option<u32>) -> Result<(), PlatformError> {
        let _ = (path, mode);
        Err(PlatformError::Unsupported("create_dir"))
    }

    /// Set permission bits (`0o777` only) on a file or directory; fails on a
    /// symlink instead of changing its target.
    fn set_mode(&self, path: &Path, mode: u32) -> Result<(), PlatformError> {
        let _ = (path, mode);
        Err(PlatformError::Unsupported("set_mode"))
    }

    /// Create a symlink at `link` whose target string is `target` verbatim.
    fn create_symlink(&self, target: &str, link: &Path) -> Result<(), PlatformError> {
        let _ = (target, link);
        Err(PlatformError::Unsupported("create_symlink"))
    }

    /// Raw target of the symlink at `path` (never resolved).
    fn read_link(&self, path: &Path) -> Result<PathBuf, PlatformError> {
        let _ = path;
        Err(PlatformError::Unsupported("read_link"))
    }

    /// Atomically rename `from` to `to`, failing if `to` exists
    /// (Linux `renameat2(RENAME_NOREPLACE)`). Cross-filesystem moves fail
    /// with an `Io` error of kind `CrossesDevices`.
    fn rename_noreplace(&self, from: &Path, to: &Path) -> Result<(), PlatformError> {
        let _ = (from, to);
        Err(PlatformError::Unsupported("rename_noreplace"))
    }

    /// Atomically rename `from` to `to`, replacing `to` if it exists (the
    /// last step of an atomic document write). Never follows a symlink at
    /// either path: a link is renamed or replaced as itself.
    fn rename_replace(&self, from: &Path, to: &Path) -> Result<(), PlatformError> {
        let _ = (from, to);
        Err(PlatformError::Unsupported("rename_replace"))
    }

    /// Flush a directory's entries to disk (fsync on the directory).
    fn sync_dir(&self, path: &Path) -> Result<(), PlatformError> {
        let _ = path;
        Err(PlatformError::Unsupported("sync_dir"))
    }

    /// Remove a file or symlink (never a directory; never follows).
    fn remove_file(&self, path: &Path) -> Result<(), PlatformError> {
        let _ = path;
        Err(PlatformError::Unsupported("remove_file"))
    }

    /// Entries of the directory at `path`, sorted by name, without `.` and
    /// `..`. Fails if `path` itself is a symlink (`O_NOFOLLOW`) or not a
    /// directory; entry kinds use lstat semantics (links are reported, never
    /// followed).
    fn read_dir(&self, path: &Path) -> Result<Vec<DirEntry>, PlatformError> {
        let _ = path;
        Err(PlatformError::Unsupported("read_dir"))
    }

    /// Recursively delete `path`, which must lie strictly inside `within`
    /// (lexically, after normalisation). Never follows symlinks: links are
    /// removed, not traversed.
    fn remove_dir_all_confined(&self, path: &Path, within: &Path) -> Result<(), PlatformError> {
        let _ = (path, within);
        Err(PlatformError::Unsupported("remove_dir_all_confined"))
    }

    // ---- Phase 3: staging fast path ---------------------------------------
    //
    // Fd-based methods return `std::io::Result` (there is no path to
    // report); their defaults fail with `io::ErrorKind::Unsupported`.

    /// Open an existing regular file for reading and writing (no create,
    /// no truncate); fails if `path` is a symlink (`O_NOFOLLOW`), missing
    /// (`NotFound`) or not a regular file (checked with `fstat` on the
    /// opened fd). Used to reopen Carrova's own staged files on resume.
    fn open_rw_nofollow(&self, path: &Path) -> Result<File, PlatformError> {
        let _ = path;
        Err(PlatformError::Unsupported("open_rw_nofollow"))
    }

    /// Identity of the filesystem holding `path` (**follows** symlinks:
    /// the answer is where data written through `path` lands).
    fn fs_id(&self, path: &Path) -> Result<FsId, PlatformError> {
        let _ = path;
        Err(PlatformError::Unsupported("fs_id"))
    }

    /// Flush every dirty page and inode of the filesystem holding `path`
    /// (Linux `syncfs`), the cheap way to make many written files durable
    /// at once. `path` (a directory or regular file) is opened without
    /// following a final symlink. Callers fall back to per-file
    /// `File::sync_data` on [`PlatformError::Unsupported`].
    fn sync_fs(&self, path: &Path) -> Result<(), PlatformError> {
        let _ = path;
        Err(PlatformError::Unsupported("sync_fs"))
    }

    /// Write all of `buf` at byte `offset` of `file` without moving its
    /// cursor (positional write, `pwrite`).
    fn write_all_at(&self, file: &File, buf: &[u8], offset: u64) -> io::Result<()> {
        let _ = (file, buf, offset);
        Err(io::Error::new(io::ErrorKind::Unsupported, "write_all_at"))
    }

    /// Size a freshly created, empty `file` to `len` bytes, reserving the
    /// blocks where the filesystem can ([`Allocation::Reserved`]) so a full
    /// disk fails here with `io::ErrorKind::StorageFull` rather than in a
    /// later write; otherwise the file is extended sparsely
    /// ([`Allocation::Sparse`]).
    fn preallocate(&self, file: &File, len: u64) -> io::Result<Allocation> {
        let _ = (file, len);
        Err(io::Error::new(io::ErrorKind::Unsupported, "preallocate"))
    }

    /// Metadata of an open `file` (`fstat`): describes the inode actually
    /// opened, so a path swapped after the open cannot change the answer.
    /// Use it to vet a file before trusting what is read from it.
    fn file_metadata(&self, file: &File) -> io::Result<FileMeta> {
        let _ = file;
        Err(io::Error::new(io::ErrorKind::Unsupported, "file_metadata"))
    }

    /// Take an exclusive advisory lock on the file at `path` (created,
    /// mode `0600`, if missing; never through a symlink), blocking until
    /// it is free. Released when the returned guard drops. Serializes
    /// other processes and other threads of this process alike (Linux:
    /// `flock`).
    fn lock_exclusive(&self, path: &Path) -> Result<FileLock, PlatformError> {
        let _ = path;
        Err(PlatformError::Unsupported("lock_exclusive"))
    }

    /// Effective user id of this process, where the OS has one (Linux).
    /// Core uses it to refuse secrets (e.g. `resume.key`) owned by another
    /// user, compared against [`FileMeta::uid`].
    fn current_uid(&self) -> Option<u32> {
        None
    }

    /// This computer's name as the user knows it (Linux: the kernel host
    /// name), sanitised for display: printable characters only, trimmed,
    /// at most 64 characters. `None` when the OS has no usable name.
    fn device_name(&self) -> Option<String> {
        None
    }

    // ---- Phase 3: LAN networking -------------------------------------------
    //
    // IPv4 only. Sockets are returned non-blocking, ready for an async
    // runtime to adopt.

    /// Usable LAN IPv4 interfaces: up, not loopback, not link-local.
    fn lan_ipv4_interfaces(&self) -> Result<Vec<LanInterface>, PlatformError> {
        Err(PlatformError::Unsupported("lan_ipv4_interfaces"))
    }

    /// Bind a broadcast-capable UDP socket on `port` of every IPv4
    /// interface that other processes on this host can bind as well (so
    /// several senders share a well-known discovery port and each receives
    /// every broadcast). `port` 0 picks a free port.
    fn bind_broadcast_udp(&self, port: u16) -> io::Result<UdpSocket> {
        let _ = port;
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "bind_broadcast_udp",
        ))
    }

    /// Bind a UDP socket on `port` of every IPv4 interface (0 = any free
    /// port), requesting `buffer` bytes of send and receive buffer. The OS
    /// may grant less; the returned [`SocketBuffers`] say what it granted.
    fn bind_udp(&self, port: u16, buffer: usize) -> io::Result<(UdpSocket, SocketBuffers)> {
        let _ = (port, buffer);
        Err(io::Error::new(io::ErrorKind::Unsupported, "bind_udp"))
    }

    /// User-facing advice for when [`Platform::bind_udp`] granted less than
    /// `wanted` bytes of buffer, if the OS has a known remedy.
    fn udp_buffer_hint(&self, wanted: usize) -> Option<String> {
        let _ = wanted;
        None
    }

    // ---- External tools (built-in handlers) ------------------------------

    /// GnuPG home directory of the current user (Linux: always
    /// `<home>/.gnupg`; an inherited `$GNUPGHOME` is deliberately ignored
    /// so imports land in a deterministic, plan-visible place).
    fn gnupg_home(&self) -> Result<PathBuf, PlatformError> {
        Err(PlatformError::Unsupported("gnupg_home"))
    }

    /// A [`Command`] for the executable `bin` with a minimal,
    /// OS-appropriate environment and no inherited variables (Linux:
    /// `HOME`, `PATH` from the adapter's snapshot, `LC_ALL=C`). Callers add
    /// only tool-specific variables and arguments; never run via a shell.
    fn tool_command(&self, bin: &Path) -> Result<Command, PlatformError> {
        let _ = bin;
        Err(PlatformError::Unsupported("tool_command"))
    }
}
