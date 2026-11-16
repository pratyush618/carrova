//! [`FakePlatform`] that also records what the supervisor opens and rings
//! (the fake leaves `open_url`, `open_path` and `ring` unsupported), and
//! plays a scripted phone: events, notes, SMS, calls, call audio.

use std::fmt;
use std::fs::File;
use std::io;
use std::net::UdpSocket;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::{self, ThreadId};
use std::time::Duration;

use carrova_platform::fake::FakePlatform;
use carrova_platform::phone::{
    AudioSide, CallAudioStatus, CallRecord, CallState, Cursor, Note, PermState, PhoneEvent,
    PhonePerm, PhoneWatch, SendOutcome, SmsMessage, SmsThread,
};
use carrova_platform::{
    ClipContent, ClipboardWatch, DirEntry, DirWatch, FileLock, FileMeta, LanInterface, LinkNotice,
    LocalListener, LocalStream, OsKind, PermissionModel, Platform, PlatformError, ProcessInfo,
    SemanticRoot, SnapshotCapability, SocketBuffers,
};

/// What a [`Recording`] platform was asked to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Done {
    OpenUrl(String),
    OpenPath(PathBuf),
    Ring(u16),
    StopRing,
}

#[derive(Debug)]
pub struct Recording {
    pub inner: FakePlatform,
    done: Mutex<Vec<Done>>,
    /// Plays an OS without local sockets (Windows, macOS today).
    no_local_socket: AtomicBool,
    /// The scripted phone's data; tests fill it through [`Recording::phone`].
    phone: Mutex<Phone>,
    events: Arc<Mutex<Events>>,
}

/// What the scripted phone holds and answers with.
#[derive(Debug)]
pub struct Phone {
    pub notes: Vec<Note>,
    pub threads: Vec<SmsThread>,
    pub messages: Vec<SmsMessage>,
    pub history: Vec<CallRecord>,
    pub send_outcome: SendOutcome,
    /// Permissions not listed are granted.
    pub perms: Vec<(PhonePerm, PermState)>,
    pub audio: CallAudioStatus,
    /// What `call_state` reports.
    pub call: CallState,
    /// Method names and ids of every phone call made, never content.
    actions: Vec<String>,
}

impl Default for Phone {
    fn default() -> Self {
        Self {
            notes: Vec::new(),
            threads: Vec::new(),
            messages: Vec::new(),
            history: Vec::new(),
            send_outcome: SendOutcome::Sent,
            perms: Vec::new(),
            audio: CallAudioStatus::Ready,
            call: CallState::Idle,
            actions: Vec::new(),
        }
    }
}

/// The live watch's delivery queue, or the events scripted before one is set.
#[derive(Default)]
struct Events {
    /// Feeds the live watch's own thread, which runs its callback.
    live: Option<Sender<PhoneEvent>>,
    pending: Vec<PhoneEvent>,
    /// Bumped by each watch and its drop, so a stale watch delivers nothing.
    generation: u64,
    /// The live watch's thread, to wait out when the watch stops.
    worker: Option<Worker>,
}

/// A watch's delivery thread. It holds `delivery` across each generation
/// check and callback, so a stop that bumps the generation and then takes
/// `delivery` knows no callback of that watch runs, now or later.
struct Worker {
    delivery: Arc<Mutex<()>>,
    thread: ThreadId,
}

impl Worker {
    /// Wait for a running callback to end, unless called from inside it
    /// (a callback that drops or replaces its own watch).
    fn finish(self) {
        if thread::current().id() != self.thread {
            drop(lock(&self.delivery));
        }
    }
}

impl fmt::Debug for Events {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Events")
            .field("watched", &self.live.is_some())
            .field("pending", &self.pending.len())
            .finish()
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Newest first by `(when_ms, id)`, strictly before `before`, at most `limit`.
fn page<T: Clone>(
    items: impl Iterator<Item = T>,
    key: impl Fn(&T) -> Cursor,
    before: Option<Cursor>,
    limit: usize,
) -> Vec<T> {
    let mut out: Vec<T> = items
        .filter(|t| before.is_none_or(|b| key(t) < b))
        .collect();
    out.sort_by_key(|t| std::cmp::Reverse(key(t)));
    out.truncate(limit);
    out
}

impl Recording {
    pub fn new(inner: FakePlatform) -> Self {
        Self {
            inner,
            done: Mutex::default(),
            no_local_socket: AtomicBool::new(false),
            phone: Mutex::default(),
            events: Arc::default(),
        }
    }

    /// The scripted phone, to fill or inspect.
    pub fn phone(&self) -> MutexGuard<'_, Phone> {
        lock(&self.phone)
    }

    /// Deliver `event` to the `watch_phone_events` callback, or hold it
    /// until one is registered.
    pub fn script_event(&self, event: PhoneEvent) {
        let mut events = lock(&self.events);
        let unsent = match &events.live {
            Some(live) => live.send(event).err().map(|e| e.0),
            None => Some(event),
        };
        events.pending.extend(unsent);
    }

    /// Whether a `watch_phone_events` callback is registered now.
    pub fn watching(&self) -> bool {
        lock(&self.events).on_event.is_some()
    }

    /// Phone methods called so far: names and ids only.
    pub fn actions(&self) -> Vec<String> {
        self.phone().actions.clone()
    }

    /// Record `action`, then hand out the phone to answer it.
    fn act(&self, action: String) -> MutexGuard<'_, Phone> {
        let mut phone = self.phone();
        phone.actions.push(action);
        phone
    }

    fn record_action(&self, action: &str) -> Result<(), PlatformError> {
        self.phone().actions.push(action.to_owned());
        Ok(())
    }

    pub fn refuse_local_sockets(&self) {
        self.no_local_socket.store(true, Ordering::SeqCst);
    }

    pub fn done(&self) -> Vec<Done> {
        self.done
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn record(&self, done: Done) -> Result<(), PlatformError> {
        self.done
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(done);
        Ok(())
    }
}

impl Platform for Recording {
    fn os(&self) -> OsKind {
        self.inner.os()
    }
    fn permission_model(&self) -> PermissionModel {
        self.inner.permission_model()
    }
    fn resolve_root(&self, root: SemanticRoot) -> Result<PathBuf, PlatformError> {
        self.inner.resolve_root(root)
    }
    fn find_binary(&self, name: &str) -> Result<Option<PathBuf>, PlatformError> {
        self.inner.find_binary(name)
    }
    fn metadata(&self, path: &Path) -> Result<FileMeta, PlatformError> {
        self.inner.metadata(path)
    }
    fn running_processes(&self) -> Result<Vec<ProcessInfo>, PlatformError> {
        self.inner.running_processes()
    }
    fn free_space(&self, path: &Path) -> Result<u64, PlatformError> {
        self.inner.free_space(path)
    }
    fn snapshot_capability(&self, path: &Path) -> Result<SnapshotCapability, PlatformError> {
        self.inner.snapshot_capability(path)
    }
    fn state_dir(&self) -> Result<PathBuf, PlatformError> {
        self.inner.state_dir()
    }
    fn open_read_nofollow(&self, path: &Path) -> Result<File, PlatformError> {
        self.inner.open_read_nofollow(path)
    }
    fn open_append_nofollow(&self, path: &Path) -> Result<File, PlatformError> {
        self.inner.open_append_nofollow(path)
    }
    fn create_new_file(&self, path: &Path, mode: Option<u32>) -> Result<File, PlatformError> {
        self.inner.create_new_file(path, mode)
    }
    fn create_dir(&self, path: &Path, mode: Option<u32>) -> Result<(), PlatformError> {
        self.inner.create_dir(path, mode)
    }
    fn set_mode(&self, path: &Path, mode: u32) -> Result<(), PlatformError> {
        self.inner.set_mode(path, mode)
    }
    fn read_link(&self, path: &Path) -> Result<PathBuf, PlatformError> {
        self.inner.read_link(path)
    }
    fn rename_noreplace(&self, from: &Path, to: &Path) -> Result<(), PlatformError> {
        self.inner.rename_noreplace(from, to)
    }
    fn rename_replace(&self, from: &Path, to: &Path) -> Result<(), PlatformError> {
        self.inner.rename_replace(from, to)
    }
    fn sync_dir(&self, path: &Path) -> Result<(), PlatformError> {
        self.inner.sync_dir(path)
    }
    fn remove_file(&self, path: &Path) -> Result<(), PlatformError> {
        self.inner.remove_file(path)
    }
    fn read_dir(&self, path: &Path) -> Result<Vec<DirEntry>, PlatformError> {
        self.inner.read_dir(path)
    }
    fn remove_dir_all_confined(&self, path: &Path, within: &Path) -> Result<(), PlatformError> {
        self.inner.remove_dir_all_confined(path, within)
    }
    fn open_rw_nofollow(&self, path: &Path) -> Result<File, PlatformError> {
        self.inner.open_rw_nofollow(path)
    }
    fn write_all_at(&self, file: &File, buf: &[u8], offset: u64) -> io::Result<()> {
        self.inner.write_all_at(file, buf, offset)
    }
    fn file_metadata(&self, file: &File) -> io::Result<FileMeta> {
        self.inner.file_metadata(file)
    }
    fn lock_exclusive(&self, path: &Path) -> Result<FileLock, PlatformError> {
        self.inner.lock_exclusive(path)
    }
    fn try_lock_exclusive(&self, path: &Path) -> Result<Option<FileLock>, PlatformError> {
        self.inner.try_lock_exclusive(path)
    }
    fn bind_local_socket(&self, path: &Path) -> Result<LocalListener, PlatformError> {
        if self.no_local_socket.load(Ordering::SeqCst) {
            return Err(PlatformError::Unsupported("bind_local_socket"));
        }
        self.inner.bind_local_socket(path)
    }
    fn connect_local_socket(&self, path: &Path) -> Result<LocalStream, PlatformError> {
        self.inner.connect_local_socket(path)
    }
    fn current_uid(&self) -> Option<u32> {
        self.inner.current_uid()
    }
    fn watch_dirs(
        &self,
        dirs: &[PathBuf],
        changed: Box<dyn Fn() + Send>,
    ) -> Result<DirWatch, PlatformError> {
        self.inner.watch_dirs(dirs, changed)
    }
    fn device_name(&self) -> Option<String> {
        self.inner.device_name()
    }
    fn lan_ipv4_interfaces(&self) -> Result<Vec<LanInterface>, PlatformError> {
        self.inner.lan_ipv4_interfaces()
    }
    fn bind_broadcast_udp(&self, port: u16) -> io::Result<UdpSocket> {
        self.inner.bind_broadcast_udp(port)
    }
    fn bind_udp(&self, port: u16, buffer: usize) -> io::Result<(UdpSocket, SocketBuffers)> {
        self.inner.bind_udp(port, buffer)
    }
    fn clipboard_read(&self) -> Result<Option<ClipContent>, PlatformError> {
        self.inner.clipboard_read()
    }
    fn clipboard_write(&self, content: &ClipContent) -> Result<(), PlatformError> {
        self.inner.clipboard_write(content)
    }
    fn watch_clipboard(
        &self,
        on_change: Box<dyn Fn() + Send>,
    ) -> Result<ClipboardWatch, PlatformError> {
        self.inner.watch_clipboard(on_change)
    }
    fn downloads_dir(&self) -> Result<PathBuf, PlatformError> {
        self.inner.downloads_dir()
    }
    fn open_url(&self, url: &str) -> Result<(), PlatformError> {
        self.record(Done::OpenUrl(url.to_owned()))
    }
    fn open_path(&self, path: &Path) -> Result<(), PlatformError> {
        self.record(Done::OpenPath(path.to_owned()))
    }
    fn ring(&self, seconds: u16) -> Result<(), PlatformError> {
        self.record(Done::Ring(seconds))
    }
    fn stop_ring(&self) -> Result<(), PlatformError> {
        self.record(Done::StopRing)
    }
    fn link_keepalive(&self) -> Duration {
        self.inner.link_keepalive()
    }
    fn keep_running(&self, notice: Option<&LinkNotice>) -> Result<(), PlatformError> {
        self.inner.keep_running(notice)
    }
    fn links_paused(&self) -> bool {
        self.inner.links_paused()
    }
    fn watch_phone_events(
        &self,
        on_event: Box<dyn Fn(PhoneEvent) + Send>,
    ) -> Result<PhoneWatch, PlatformError> {
        self.record_action("watch_phone_events")?;
        let (live, queue) = mpsc::channel();
        let delivery = Arc::new(Mutex::new(()));
        let (generation, replaced) = {
            let mut events = lock(&self.events);
            events.generation += 1; // the replaced watch stops now
            for event in std::mem::take(&mut events.pending) {
                let _ = live.send(event); // `queue` is alive: cannot fail
            }
            events.live = Some(live);
            let generation = events.generation;
            // Deliver on the watch's own thread and never under `events`:
            // the scripting thread or a supervisor may hold a lock the
            // callback takes, and a callback may drop its own watch. The
            // thread ends once the watch is dropped or replaced (its
            // sender goes).
            let (gate, held) = (Arc::clone(&self.events), Arc::clone(&delivery));
            let thread = thread::spawn(move || {
                for event in queue {
                    let _delivering = lock(&held);
                    if lock(&gate).generation != generation {
                        return;
                    }
                    on_event(event);
                }
            })
            .thread()
            .id();
            let replaced = events.worker.replace(Worker { delivery, thread });
            (generation, replaced)
        };
        // Waited on outside `events`: the worker takes it under `delivery`.
        if let Some(worker) = replaced {
            worker.finish();
        }
        let events = Arc::clone(&self.events);
        Ok(PhoneWatch::new(move || {
            let stopped = {
                let mut events = lock(&events);
                if events.generation == generation {
                    events.generation += 1; // queued events stay undelivered
                    events.live = None;
                    events.worker.take()
                } else {
                    None // replaced: the replacing call waited already
                }
            };
            if let Some(worker) = stopped {
                worker.finish();
            }
        }))
    }
    fn active_notes(&self) -> Result<Vec<Note>, PlatformError> {
        Ok(self.act("active_notes".into()).notes.clone())
    }
    fn note_reply(&self, key: &str, _text: &str) -> Result<bool, PlatformError> {
        let phone = self.act(format!("note_reply {key}"));
        Ok(phone.notes.iter().any(|n| n.key == key && n.can_reply))
    }
    fn note_dismiss(&self, key: &str) -> Result<(), PlatformError> {
        self.act(format!("note_dismiss {key}"))
            .notes
            .retain(|n| n.key != key);
        Ok(())
    }
    fn note_open(&self, key: &str) -> Result<(), PlatformError> {
        self.record_action(&format!("note_open {key}"))
    }
    fn sms_threads(
        &self,
        before: Option<Cursor>,
        limit: usize,
        since_ms: Option<i64>,
    ) -> Result<Vec<SmsThread>, PlatformError> {
        let phone = self.act("sms_threads".into());
        let fresh = phone
            .threads
            .iter()
            .filter(|t| since_ms.is_none_or(|s| t.when_ms > s))
            .cloned();
        Ok(page(
            fresh,
            |t| Cursor {
                when_ms: t.when_ms,
                id: t.id,
            },
            before,
            limit,
        ))
    }
    fn sms_thread(
        &self,
        id: i64,
        before: Option<Cursor>,
        limit: usize,
    ) -> Result<Vec<SmsMessage>, PlatformError> {
        let phone = self.act(format!("sms_thread {id}"));
        let thread = phone.messages.iter().filter(|m| m.thread_id == id).cloned();
        Ok(page(
            thread,
            |m| Cursor {
                when_ms: m.when_ms,
                id: m.id,
            },
            before,
            limit,
        ))
    }
    fn sms_send(&self, _to: &str, _text: &str) -> Result<SendOutcome, PlatformError> {
        Ok(self.act("sms_send".into()).send_outcome)
    }
    fn call_answer(&self) -> Result<(), PlatformError> {
        self.record_action("call_answer")
    }
    fn call_decline(&self) -> Result<(), PlatformError> {
        self.record_action("call_decline")
    }
    fn call_end(&self) -> Result<(), PlatformError> {
        self.record_action("call_end")
    }
    fn call_mute(&self, on: bool) -> Result<(), PlatformError> {
        self.record_action(&format!("call_mute {on}"))
    }
    fn call_dial(&self, _number: &str) -> Result<(), PlatformError> {
        self.record_action("call_dial")
    }
    fn call_speaker(&self, on: bool) -> Result<(), PlatformError> {
        self.record_action(&format!("call_speaker {on}"))
    }
    fn call_history(
        &self,
        before: Option<Cursor>,
        limit: usize,
    ) -> Result<Vec<CallRecord>, PlatformError> {
        let phone = self.act("call_history".into());
        Ok(page(
            phone.history.iter().cloned(),
            |c| Cursor {
                when_ms: c.when_ms,
                id: c.id,
            },
            before,
            limit,
        ))
    }
    fn call_state(&self) -> Result<CallState, PlatformError> {
        Ok(self.act("call_state".into()).call.clone())
    }
    fn phone_perm(&self, p: PhonePerm) -> Result<PermState, PlatformError> {
        let phone = self.act(format!("phone_perm {p:?}"));
        Ok(phone
            .perms
            .iter()
            .find_map(|&(q, state)| (q == p).then_some(state))
            .unwrap_or(PermState::Granted))
    }
    fn open_perm_settings(&self, p: PhonePerm) -> Result<(), PlatformError> {
        self.record_action(&format!("open_perm_settings {p:?}"))
    }
    fn call_audio_status(&self) -> Result<CallAudioStatus, PlatformError> {
        Ok(self.act("call_audio_status".into()).audio)
    }
    fn install_call_audio(&self) -> Result<(), PlatformError> {
        self.record_action("install_call_audio")
    }
    fn set_call_audio_config(&self, on: bool) -> Result<(), PlatformError> {
        self.record_action(&format!("set_call_audio_config {on}"))
    }
    fn bt_pair(&self, _on_passkey: Box<dyn Fn(u32) + Send>) -> Result<(), PlatformError> {
        self.record_action("bt_pair")
    }
    fn call_tones(&self, _digits: &str) -> Result<(), PlatformError> {
        self.record_action("call_tones")
    }
    fn call_audio_to(&self, side: AudioSide) -> Result<(), PlatformError> {
        self.record_action(&format!("call_audio_to {side:?}"))
    }
}
