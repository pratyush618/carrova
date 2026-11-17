//! The [`Platform`] trait: every OS interaction goes through it.

use std::fs::File;
use std::io;
use std::net::{IpAddr, UdpSocket};
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::Duration;

use crate::phone::{
    AudioSide, CallAudioStatus, CallRecord, CallState, Cursor, Note, PermState, PhoneEvent,
    PhonePerm, PhoneWatch, SendOutcome, Sim, SmsMessage, SmsThread,
};
use crate::{
    Allocation, ClipContent, ClipboardWatch, DirEntry, DirWatch, DiskInfo, DownloadSink, FileLock,
    FileManagerStatus, FileMeta, FsId, LanInterface, LinkNotice, LocalListener, LocalStream,
    MulticastGuard, OsKind, PermissionModel, PlatformError, ProcessInfo, ScreenLock, SemanticRoot,
    SnapshotCapability, SocketBuffers,
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

    /// Size and mount point of the filesystem holding `path` (**follows**
    /// symlinks, like [`Platform::free_space`]), so a low-space message can
    /// name the disk and show how full it is.
    fn disk(&self, path: &Path) -> Result<DiskInfo, PlatformError> {
        let _ = path;
        Err(PlatformError::Unsupported("disk"))
    }

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

    /// [`Self::lock_exclusive`] without waiting: `None` while another
    /// holder has it.
    fn try_lock_exclusive(&self, path: &Path) -> Result<Option<FileLock>, PlatformError> {
        let _ = path;
        Err(PlatformError::Unsupported("try_lock_exclusive"))
    }

    /// Effective user id of this process, where the OS has one (Linux).
    /// Core uses it to refuse secrets (e.g. `resume.key`) owned by another
    /// user, compared against [`FileMeta::uid`].
    fn current_uid(&self) -> Option<u32> {
        None
    }

    /// Call `changed` (on a thread the watch owns) whenever an entry of one
    /// of `dirs` is created, replaced, written or removed, until the
    /// returned guard drops. Entries are not followed into: watch the
    /// directory of a file replaced by rename. A dir missing now is watched
    /// from when it appears in another watched dir; a dir that is not one
    /// is refused. If watching fails later (a dir refused, or none of
    /// `dirs` left), the watch ends by itself and drops `changed`: a caller
    /// that must know keeps a guard in it.
    fn watch_dirs(
        &self,
        dirs: &[PathBuf],
        changed: Box<dyn Fn() + Send>,
    ) -> Result<DirWatch, PlatformError> {
        let _ = (dirs, changed);
        Err(PlatformError::Unsupported("watch_dirs"))
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

    /// Whether this device is on a local network other devices can share
    /// now (the phone's "not on Wi-Fi" check before pairing). By default,
    /// any usable LAN address; Android asks the system for the active
    /// network and counts only Wi-Fi or Ethernet (mobile data has private
    /// addresses too).
    fn on_local_network(&self) -> Result<bool, PlatformError> {
        Ok(!self.lan_ipv4_interfaces()?.is_empty())
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

    /// Hardware address the OS has cached for the LAN host `addr` (Linux:
    /// the kernel neighbour table; macOS and Windows: the `arp` tool), if it
    /// has talked to it lately. IPv4 only. Used to remember a device's
    /// address so it can be woken later.
    fn neighbour_mac(&self, addr: IpAddr) -> Option<[u8; 6]> {
        let _ = addr;
        None
    }

    /// Broadcast a Wake-on-LAN magic packet for `mac` (UDP, port 9). Best
    /// effort: a delivered packet says nothing about the target waking.
    fn send_magic_packet(&self, mac: [u8; 6]) -> Result<(), PlatformError> {
        let _ = mac;
        Err(PlatformError::Unsupported("send_magic_packet"))
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

    /// Make the tool `cmd` starts the head of its own process tree (Unix:
    /// a new process group), so [`Platform::kill_tool`] also ends whatever
    /// it spawned. Only for tools that never read the terminal.
    fn isolate_tool(&self, cmd: &mut Command) {
        crate::sys::isolate_tool(cmd);
    }

    /// Kill `child`, started from an [`isolated`](Platform::isolate_tool)
    /// command, with every process still in its tree. Call before reaping
    /// `child`, while its id cannot be reused.
    fn kill_tool(&self, child: &mut Child) -> io::Result<()> {
        crate::sys::kill_tool(child)
    }

    // ---- File-manager entries ("Send with Carrova") ----------------------

    /// The executable file-manager entries launch (Linux AppImage: the
    /// image, not its mount).
    fn launcher(&self) -> Result<PathBuf, PlatformError> {
        std::env::current_exe().map_err(|e| PlatformError::io("<current executable>", e))
    }

    /// Whether Carrova's file-manager entries are written for this user.
    fn file_manager_status(&self) -> Result<FileManagerStatus, PlatformError> {
        Ok(FileManagerStatus::default())
    }

    /// Write (or refresh) this user's file-manager entries, each launching
    /// `launcher` with `--send` and the chosen paths.
    fn install_file_manager_entries(
        &self,
        launcher: &Path,
    ) -> Result<FileManagerStatus, PlatformError> {
        let _ = launcher;
        Err(PlatformError::Unsupported("install_file_manager_entries"))
    }

    /// Remove every file-manager entry Carrova wrote, and only those.
    fn remove_file_manager_entries(&self) -> Result<FileManagerStatus, PlatformError> {
        Err(PlatformError::Unsupported("remove_file_manager_entries"))
    }

    // ---- Notifications ------------------------------------------------------

    /// Light the display up, as if the user had touched the computer, so an
    /// alert can be seen. Best effort: callers log a failure at debug level
    /// and carry on. Defaults to [`PlatformError::Unsupported`]: adapters
    /// without an unsafe-free way to do it stay honest.
    fn wake_display(&self) -> Result<(), PlatformError> {
        Err(PlatformError::Unsupported("wake_display"))
    }

    /// Whether the screen is locked now. [`ScreenLock::Unknown`] when the
    /// OS cannot say; callers then fail closed (hide what a locked screen
    /// must not show).
    fn screen_lock(&self) -> ScreenLock {
        ScreenLock::Unknown
    }

    // ---- Background ---------------------------------------------------------

    /// Whether the desktop shows status-tray icons, so a closed window can
    /// leave one behind (Linux: a StatusNotifier host is registered; plain
    /// GNOME has none). Asked at each close: hosts come and go.
    fn has_status_tray(&self) -> bool {
        true
    }

    /// Whether Carrova starts in the background when this user signs in.
    fn autostart_status(&self) -> Result<bool, PlatformError> {
        Ok(false)
    }

    /// Start `launcher --background` when this user signs in (`on`), or
    /// stop doing so; replies with [`Platform::autostart_status`] after the
    /// change. Turning off what is already off succeeds.
    fn set_autostart(&self, launcher: &Path, on: bool) -> Result<bool, PlatformError> {
        let _ = (launcher, on);
        Err(PlatformError::Unsupported("set_autostart"))
    }

    // ---- Ecosystem Phase 1: clipboard, open, local socket ----------------
    // Default bodies return `Unsupported` so stub adapters stay honest.

    /// The clipboard's content; `None` when it is empty or holds nothing
    /// Carrova syncs (text and PNG).
    fn clipboard_read(&self) -> Result<Option<ClipContent>, PlatformError> {
        Err(PlatformError::Unsupported("clipboard_read"))
    }

    /// Replace the clipboard's content.
    fn clipboard_write(&self, content: &ClipContent) -> Result<(), PlatformError> {
        let _ = content;
        Err(PlatformError::Unsupported("clipboard_write"))
    }

    /// Call `on_change` (on a thread the watch owns) whenever the clipboard's
    /// content changes, until the returned guard drops.
    fn watch_clipboard(
        &self,
        on_change: Box<dyn Fn() + Send>,
    ) -> Result<ClipboardWatch, PlatformError> {
        let _ = on_change;
        Err(PlatformError::Unsupported("watch_clipboard"))
    }

    /// Where the user keeps downloads.
    fn downloads_dir(&self) -> Result<PathBuf, PlatformError> {
        Err(PlatformError::Unsupported("downloads_dir"))
    }

    /// A new file named like `name` (type `mime`) that the OS itself saves
    /// into Downloads, where apps may not write paths directly (Android's
    /// MediaStore). `Unsupported` where [`Platform::downloads_dir`] is
    /// writable: callers write there instead.
    fn downloads_sink(
        &self,
        name: &str,
        mime: &str,
    ) -> Result<Box<dyn DownloadSink>, PlatformError> {
        let _ = (name, mime);
        Err(PlatformError::Unsupported("downloads_sink"))
    }

    /// Open an `http(s)` URL in the user's browser; any other scheme is
    /// [`PlatformError::InvalidInput`].
    fn open_url(&self, url: &str) -> Result<(), PlatformError> {
        let _ = url;
        Err(PlatformError::Unsupported("open_url"))
    }

    /// Open an absolute path with the user's default application.
    fn open_path(&self, path: &Path) -> Result<(), PlatformError> {
        let _ = path;
        Err(PlatformError::Unsupported("open_path"))
    }

    /// Make this device ring for `seconds` (phones only: a computer cannot).
    fn ring(&self, seconds: u16) -> Result<(), PlatformError> {
        let _ = seconds;
        Err(PlatformError::Unsupported("ring"))
    }

    /// End a [`Platform::ring`] early.
    fn stop_ring(&self) -> Result<(), PlatformError> {
        Err(PlatformError::Unsupported("stop_ring"))
    }

    /// Keep multicast traffic (mDNS) flowing while the guard lives. A no-op
    /// where the OS never filters it.
    fn multicast_lock(&self) -> Result<MulticastGuard, PlatformError> {
        Ok(MulticastGuard::unfiltered())
    }

    /// Listen on a Unix socket at `path` (mode `0600`); only peers with
    /// this process's [`Platform::current_uid`] are served.
    fn bind_local_socket(&self, path: &Path) -> Result<LocalListener, PlatformError> {
        let _ = path;
        Err(PlatformError::Unsupported("bind_local_socket"))
    }

    /// Connect to a socket bound by [`Platform::bind_local_socket`].
    fn connect_local_socket(&self, path: &Path) -> Result<LocalStream, PlatformError> {
        let _ = path;
        Err(PlatformError::Unsupported("connect_local_socket"))
    }

    /// How often a link must send a keepalive so the OS keeps the
    /// connection (and a mobile radio) alive.
    fn link_keepalive(&self) -> Duration {
        Duration::from_secs(15)
    }

    /// Keep this process running in the background, showing `notice`
    /// (a phone's foreground service), or with `None` let the OS stop it.
    /// A no-op where background processes are not stopped.
    fn keep_running(&self, notice: Option<&LinkNotice>) -> Result<(), PlatformError> {
        let _ = notice;
        Ok(())
    }

    /// Whether the user paused linking outside the app (a phone's
    /// notification). Never where there is no such control.
    fn links_paused(&self) -> bool {
        false
    }

    /// Scan a QR code with the camera (phones) and return its text, as
    /// is; `None` if the user cancelled.
    fn scan_qr(&self) -> Result<Option<String>, PlatformError> {
        Err(PlatformError::Unsupported("scan_qr"))
    }

    // ---- Ecosystem Phase 2: phone notifications, SMS, calls, call audio ----
    // Default bodies return `Unsupported` so stub adapters stay honest.

    /// Call `on_event` (on a thread the watch owns) with each event the
    /// phone reports, until the returned guard drops.
    ///
    /// # Errors
    /// [`PlatformError::Unsupported`] where there is no phone to watch.
    fn watch_phone_events(
        &self,
        on_event: Box<dyn Fn(PhoneEvent) + Send>,
    ) -> Result<PhoneWatch, PlatformError> {
        let _ = on_event;
        Err(PlatformError::Unsupported("watch_phone_events"))
    }

    /// The notifications posted on the phone now.
    ///
    /// # Errors
    /// [`PlatformError::Unsupported`] off a phone; OS failures otherwise.
    fn active_notes(&self) -> Result<Vec<Note>, PlatformError> {
        Err(PlatformError::Unsupported("active_notes"))
    }

    /// Reply `text` inline to the notification `key`; `false` when it is
    /// gone (or no longer offers a reply).
    ///
    /// # Errors
    /// [`PlatformError::Unsupported`] off a phone; OS failures otherwise.
    fn note_reply(&self, key: &str, text: &str) -> Result<bool, PlatformError> {
        let _ = (key, text);
        Err(PlatformError::Unsupported("note_reply"))
    }

    /// Dismiss the notification `key`; a gone one is no error.
    ///
    /// # Errors
    /// [`PlatformError::Unsupported`] off a phone; OS failures otherwise.
    fn note_dismiss(&self, key: &str) -> Result<(), PlatformError> {
        let _ = key;
        Err(PlatformError::Unsupported("note_dismiss"))
    }

    /// Open the notification `key` on the phone, as a tap would.
    ///
    /// # Errors
    /// [`PlatformError::Unsupported`] off a phone; OS failures otherwise.
    fn note_open(&self, key: &str) -> Result<(), PlatformError> {
        let _ = key;
        Err(PlatformError::Unsupported("note_open"))
    }

    /// Up to `limit` SMS threads, newest first by `(when_ms DESC, id DESC)`:
    /// only those ordered strictly before the cursor `before` (a page;
    /// exclusive on the pair, so equal times neither repeat nor vanish) and
    /// last active after `since_ms` (a refresh).
    ///
    /// # Errors
    /// [`PlatformError::Unsupported`] off a phone; OS failures otherwise.
    fn sms_threads(
        &self,
        before: Option<Cursor>,
        limit: usize,
        since_ms: Option<i64>,
    ) -> Result<Vec<SmsThread>, PlatformError> {
        let _ = (before, limit, since_ms);
        Err(PlatformError::Unsupported("sms_threads"))
    }

    /// Up to `limit` messages of thread `id`, newest first by
    /// `(when_ms DESC, id DESC)`, ordered strictly before the cursor `before`
    /// (exclusive on the pair).
    ///
    /// # Errors
    /// [`PlatformError::Unsupported`] off a phone; OS failures otherwise.
    fn sms_thread(
        &self,
        id: i64,
        before: Option<Cursor>,
        limit: usize,
    ) -> Result<Vec<SmsMessage>, PlatformError> {
        let _ = (id, before, limit);
        Err(PlatformError::Unsupported("sms_thread"))
    }

    /// The phone's active SIMs, by slot; empty when it may not read them
    /// (then texts and calls use the default SIM).
    ///
    /// # Errors
    /// [`PlatformError::Unsupported`] off a phone; OS failures otherwise.
    fn phone_sims(&self) -> Result<Vec<Sim>, PlatformError> {
        Err(PlatformError::Unsupported("phone_sims"))
    }

    /// Send `text` by SMS to the number `to`, from SIM `sim` (a
    /// [`Sim::id`]; `None`: the phone's default).
    ///
    /// # Errors
    /// [`PlatformError::Unsupported`] off a phone; OS failures otherwise
    /// (a send that merely fails is [`SendOutcome::Failed`], one with no
    /// clear answer [`SendOutcome::Unknown`]).
    fn sms_send(
        &self,
        to: &str,
        text: &str,
        sim: Option<i32>,
    ) -> Result<SendOutcome, PlatformError> {
        let _ = (to, text, sim);
        Err(PlatformError::Unsupported("sms_send"))
    }

    /// Answer the ringing call.
    ///
    /// # Errors
    /// [`PlatformError::Unsupported`] off a phone; OS failures otherwise.
    fn call_answer(&self) -> Result<(), PlatformError> {
        Err(PlatformError::Unsupported("call_answer"))
    }

    /// Decline the ringing call.
    ///
    /// # Errors
    /// [`PlatformError::Unsupported`] off a phone; OS failures otherwise.
    fn call_decline(&self) -> Result<(), PlatformError> {
        Err(PlatformError::Unsupported("call_decline"))
    }

    /// Hang up the active call.
    ///
    /// # Errors
    /// [`PlatformError::Unsupported`] off a phone; OS failures otherwise.
    fn call_end(&self) -> Result<(), PlatformError> {
        Err(PlatformError::Unsupported("call_end"))
    }

    /// Mute (`on`) or unmute the active call's microphone.
    ///
    /// # Errors
    /// [`PlatformError::Unsupported`] off a phone; OS failures otherwise.
    fn call_mute(&self, on: bool) -> Result<(), PlatformError> {
        let _ = on;
        Err(PlatformError::Unsupported("call_mute"))
    }

    /// Place a call to `number` from SIM `sim` (a [`Sim::id`]; `None`: the
    /// phone's default).
    ///
    /// # Errors
    /// [`PlatformError::Unsupported`] off a phone; OS failures otherwise.
    fn call_dial(&self, number: &str, sim: Option<i32>) -> Result<(), PlatformError> {
        let _ = (number, sim);
        Err(PlatformError::Unsupported("call_dial"))
    }

    /// Turn the phone's speaker on (`on`) or off for the active call.
    ///
    /// # Errors
    /// [`PlatformError::Unsupported`] off a phone; OS failures otherwise.
    fn call_speaker(&self, on: bool) -> Result<(), PlatformError> {
        let _ = on;
        Err(PlatformError::Unsupported("call_speaker"))
    }

    /// Up to `limit` call-log entries, newest first by
    /// `(when_ms DESC, id DESC)`, ordered strictly before the cursor `before`
    /// (exclusive on the pair).
    ///
    /// # Errors
    /// [`PlatformError::Unsupported`] off a phone; OS failures otherwise.
    fn call_history(
        &self,
        before: Option<Cursor>,
        limit: usize,
    ) -> Result<Vec<CallRecord>, PlatformError> {
        let _ = (before, limit);
        Err(PlatformError::Unsupported("call_history"))
    }

    /// The phone's call state now, to resend after events were dropped.
    ///
    /// # Errors
    /// [`PlatformError::Unsupported`] off a phone; OS failures otherwise.
    fn call_state(&self) -> Result<CallState, PlatformError> {
        Err(PlatformError::Unsupported("call_state"))
    }

    /// Whether the phone permission `p` is held.
    ///
    /// # Errors
    /// [`PlatformError::Unsupported`] off a phone.
    fn phone_perm(&self, p: PhonePerm) -> Result<PermState, PlatformError> {
        let _ = p;
        Err(PlatformError::Unsupported("phone_perm"))
    }

    /// Ask the user for `p` with the system's own prompt, then return its
    /// state. Blocks until the user answers; call off the UI thread.
    ///
    /// # Errors
    /// [`PlatformError::Unsupported`] off a phone; OS failures otherwise.
    fn request_phone_perm(&self, p: PhonePerm) -> Result<PermState, PlatformError> {
        let _ = p;
        Err(PlatformError::Unsupported("request_phone_perm"))
    }

    /// Open the system settings screen where the user grants `p`.
    ///
    /// # Errors
    /// [`PlatformError::Unsupported`] off a phone; OS failures otherwise.
    fn open_perm_settings(&self, p: PhonePerm) -> Result<(), PlatformError> {
        let _ = p;
        Err(PlatformError::Unsupported("open_perm_settings"))
    }

    /// Remind the user, with a notification on this phone, to grant `p`;
    /// tapping it opens the screen [`Self::open_perm_settings`] opens.
    ///
    /// # Errors
    /// [`PlatformError::Unsupported`] off a phone; OS failures otherwise
    /// (this app may not post notifications).
    fn remind_access(&self, p: PhonePerm) -> Result<(), PlatformError> {
        let _ = p;
        Err(PlatformError::Unsupported("remind_access"))
    }

    /// Whether this computer can carry a phone call's audio over Bluetooth.
    ///
    /// # Errors
    /// [`PlatformError::Unsupported`] where the adapter cannot tell; OS
    /// failures otherwise.
    fn call_audio_status(&self) -> Result<CallAudioStatus, PlatformError> {
        Err(PlatformError::Unsupported("call_audio_status"))
    }

    /// Install what [`CallAudioStatus::NeedsPackage`] asks for (the OS
    /// prompts the user for authorisation).
    ///
    /// # Errors
    /// [`PlatformError::Unsupported`] where call audio is not supported;
    /// install failures otherwise.
    fn install_call_audio(&self) -> Result<(), PlatformError> {
        Err(PlatformError::Unsupported("install_call_audio"))
    }

    /// Turn this computer's hands-free audio role on (`on`) or off.
    ///
    /// # Errors
    /// [`PlatformError::Unsupported`] where call audio is not supported;
    /// OS failures otherwise.
    fn set_call_audio_config(&self, on: bool) -> Result<(), PlatformError> {
        let _ = on;
        Err(PlatformError::Unsupported("set_call_audio_config"))
    }

    /// Pair the phone over Bluetooth, calling `on_passkey` with the code
    /// both screens must show.
    ///
    /// # Errors
    /// [`PlatformError::Unsupported`] where call audio is not supported;
    /// pairing failures otherwise.
    fn bt_pair(&self, on_passkey: Box<dyn Fn(u32) + Send>) -> Result<(), PlatformError> {
        let _ = on_passkey;
        Err(PlatformError::Unsupported("bt_pair"))
    }

    /// Send DTMF `digits` (`0`–`9`, `*`, `#`) on the active call.
    ///
    /// # Errors
    /// [`PlatformError::Unsupported`] where there is no call to send them
    /// on; OS failures otherwise.
    fn call_tones(&self, digits: &str) -> Result<(), PlatformError> {
        let _ = digits;
        Err(PlatformError::Unsupported("call_tones"))
    }

    /// Route the active call's audio to `side`.
    ///
    /// # Errors
    /// [`PlatformError::Unsupported`] where call audio is not supported;
    /// OS failures otherwise.
    fn call_audio_to(&self, side: AudioSide) -> Result<(), PlatformError> {
        let _ = side;
        Err(PlatformError::Unsupported("call_audio_to"))
    }

    /// Where the active call's audio plays now: [`AudioSide::Laptop`] while
    /// this computer carries the phone's audio link, else
    /// [`AudioSide::Phone`]. The live route, not whether it could.
    ///
    /// # Errors
    /// [`PlatformError::Unsupported`] where call audio is not supported;
    /// OS failures otherwise (callers then show [`AudioSide::Phone`]).
    fn call_audio_side(&self) -> Result<AudioSide, PlatformError> {
        Err(PlatformError::Unsupported("call_audio_side"))
    }
}
