//! Shared helpers for the `lan_*` integration tests: two engines (sender
//! home + receiver home, each with its own state dir) talking over
//! loopback, with event hooks to pause or "crash" either side.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::net::{Ipv4Addr, SocketAddrV4, UdpSocket};
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use carrova_core::store::Store;
use carrova_core::{
    CodeOffer, Engine, Event, EventSink, Exporter, Preset, Result, ScanOptions, SendHandle,
    SendPrompt, SendReport, SessionId, Ticket,
};
use carrova_platform::fake::FakePlatform;
use tempfile::TempDir;

#[expect(dead_code, reason = "shared test helpers; this test uses a subset")]
#[path = "apply.rs"]
pub mod apply_support;

use crate::common;
pub use apply_support::{Snapshot, snapshot};

/// The test app: config and data (default) and SSH keys (sensitive).
const RECIPE: &str = r"
schema: 2
id: app
name: The App
category: test
detect:
  linux:
    - binary: app
items:
  - id: config
    label: Config
    paths:
      - { root: user_config, rel: app }
    flags:
      default: true
  - id: data
    label: Data
    paths:
      - { root: user_data, rel: app }
    flags:
      default: true
  - id: keys
    label: Keys
    paths:
      - { root: home, rel: .ssh }
    flags:
      sensitive: true
      default: false
";

/// Size of the sender's big data file.
pub const BIG: usize = 12 * 1024 * 1024;

/// Called for every event (after it is recorded); may sleep or unwind.
pub type Hook = Box<dyn FnMut(&Event) + Send>;

/// Records events and runs an optional hook on each.
#[derive(Default)]
pub struct HookSink {
    events: Mutex<Vec<Event>>,
    hook: Mutex<Option<Hook>>,
}

impl HookSink {
    pub fn events(&self) -> Vec<Event> {
        self.events
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    pub fn set_hook(&self, hook: impl FnMut(&Event) + Send + 'static) {
        *self.hook.lock().unwrap_or_else(PoisonError::into_inner) = Some(Box::new(hook));
    }

    pub fn clear_hook(&self) {
        *self.hook.lock().unwrap_or_else(PoisonError::into_inner) = None;
    }

    /// Wait until an event matching `f` was emitted.
    pub fn wait_for(&self, what: &str, f: impl Fn(&Event) -> bool) {
        for _ in 0..600 {
            if self.events().iter().any(&f) {
                return;
            }
            thread::sleep(Duration::from_millis(50));
        }
        panic!("timed out waiting for {what}");
    }
}

impl EventSink for HookSink {
    fn emit(&self, event: Event) {
        self.events
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(event.clone());
        // Take the hook out while it runs: it may unwind ("crash").
        let hook = self
            .hook
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        if let Some(mut hook) = hook {
            hook(&event);
            let mut slot = self.hook.lock().unwrap_or_else(PoisonError::into_inner);
            if slot.is_none() {
                *slot = Some(hook);
            }
        }
    }
}

/// Unwind without the panic hook: the "process" dies.
pub fn crash() -> ! {
    std::panic::resume_unwind(Box::new("simulated crash"))
}

/// One machine: home + separate state dir.
pub struct Side {
    pub tmp: TempDir,
    pub home: PathBuf,
    pub platform: Arc<FakePlatform>,
    pub sink: Arc<HookSink>,
    pub engine: Engine,
}

impl Side {
    pub fn new() -> Self {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().join("home");
        std::fs::create_dir_all(home.join(".config")).unwrap();
        std::fs::create_dir_all(home.join(".local/share")).unwrap();
        let platform = Arc::new(
            FakePlatform::with_home(&home)
                .with_state_dir(tmp.path().join("state/carrova"))
                .binary("app", "/usr/bin/app"),
        );
        let sink = Arc::new(HookSink::default());
        let engine = Engine::new(platform.clone(), common::recipes(RECIPE), sink.clone());
        Self {
            tmp,
            home,
            platform,
            sink,
            engine,
        }
    }

    /// The sender's home: config files, a sensitive key and a big data
    /// file (several transfer batches of progress).
    pub fn sender() -> Self {
        let s = Self::new();
        s.mkdir(".config/app", 0o700);
        s.put(".config/app/settings.json", b"{\"theme\":\"dark\"}", 0o644);
        s.put(".config/app/notes.txt", b"sender notes", 0o640);
        s.mkdir(".local/share/app", 0o755);
        s.put(".local/share/app/big.bin", &pseudo_random(BIG, 7), 0o600);
        s.put(".local/share/app/small.bin", &pseudo_random(5000, 9), 0o644);
        s.mkdir(".ssh", 0o700);
        s.put(".ssh/id_ed25519", b"PRIVATE KEY BYTES", 0o600);
        s
    }

    pub fn put(&self, rel: &str, bytes: &[u8], mode: u32) {
        common::put(&*self.platform, &self.home, rel, bytes, mode);
    }

    pub fn mkdir(&self, rel: &str, mode: u32) {
        common::mkdir(&*self.platform, &self.home, rel, mode);
    }

    pub fn snap(&self) -> Snapshot {
        snapshot(&*self.platform, &self.home)
    }

    pub fn session_dir(&self, id: &SessionId) -> PathBuf {
        Store::open(&*self.platform)
            .unwrap()
            .session_dir(id)
            .unwrap()
    }

    /// An exporter with the suggested selection plus the confirmed keys.
    pub fn selected_exporter(&self) -> Exporter {
        let mut exp = Exporter::start(
            self.engine.clone(),
            ScanOptions {
                include_undetected: false,
                unrecognised: false,
            },
        )
        .unwrap();
        let mut sel = exp.suggest(Preset::Suggested);
        sel.set_included("app/keys", true).unwrap();
        sel.confirm_sensitive("app/keys").unwrap();
        exp.select(sel).unwrap();
        exp
    }

    /// `Progress` events of `phase`.
    pub fn progress(&self, phase: &str) -> Vec<(u64, u64)> {
        self.sink
            .events()
            .into_iter()
            .filter_map(|e| match e {
                Event::Progress {
                    phase: p,
                    done,
                    total,
                } if p.as_str() == phase => Some((done, total)),
                _ => None,
            })
            .collect()
    }
}

/// Deterministic incompressible-looking bytes.
pub fn pseudo_random(len: usize, seed: u64) -> Vec<u8> {
    let mut x = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    (0..len)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            (x >> 24) as u8
        })
        .collect()
}

/// `ticket` pointed at the sender's port on loopback.
pub fn loopback(ticket: &Ticket, port: u16) -> Ticket {
    Ticket {
        code: ticket.code.clone(),
        addr: Some(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port)),
    }
}

/// A running sender: its thread, the tickets it offered after failed
/// pairing attempts (already pointed at loopback) and every
/// `confirm_new_code(failures, remaining)` it asked.
pub struct Running {
    pub thread: JoinHandle<Result<SendReport>>,
    pub tickets: mpsc::Receiver<Ticket>,
    pub first: Option<Ticket>,
    pub asked: Arc<Mutex<Vec<(u32, u32)>>>,
}

impl Running {
    /// The `(failures, remaining)` questions asked so far.
    pub fn asked(&self) -> Vec<(u32, u32)> {
        asked(&self.asked)
    }

    /// Wait for the send to end: its result, the questions asked and the
    /// codes offered that were not received yet.
    pub fn finish(self) -> (Result<SendReport>, Vec<(u32, u32)>, Vec<Ticket>) {
        let result = self.thread.join().unwrap();
        (
            result,
            asked(&self.asked),
            self.tickets.try_iter().collect(),
        )
    }
}

fn asked(log: &Mutex<Vec<(u32, u32)>>) -> Vec<(u32, u32)> {
    log.lock().unwrap_or_else(PoisonError::into_inner).clone()
}

/// Test front-end: forwards renewed codes, records and answers questions.
struct TestPrompt {
    port: u16,
    tx: mpsc::Sender<Ticket>,
    answer: bool,
    asked: Arc<Mutex<Vec<(u32, u32)>>>,
}

impl SendPrompt for TestPrompt {
    fn show_code(&mut self, ticket: &Ticket, offer: CodeOffer) {
        if offer == CodeOffer::Renewed {
            let _ = self.tx.send(loopback(ticket, self.port));
        }
    }

    fn confirm_new_code(&mut self, failures: u32, remaining: u32) -> bool {
        self.asked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((failures, remaining));
        self.answer
    }
}

/// Run `handle` on a thread; the user agrees to every new code.
pub fn run(handle: SendHandle) -> Running {
    run_answering(handle, true)
}

/// Run `handle` on a thread; `answer` to every "offer a new code?".
pub fn run_answering(handle: SendHandle, answer: bool) -> Running {
    let port = handle.port();
    let first = handle.ticket().map(|t| loopback(t, port));
    let (tx, tickets) = mpsc::channel();
    let asked = Arc::new(Mutex::new(Vec::new()));
    let mut prompt = TestPrompt {
        port,
        tx,
        answer,
        asked: asked.clone(),
    };
    let thread = thread::spawn(move || handle.run(&mut prompt));
    Running {
        thread,
        tickets,
        first,
        asked,
    }
}

/// Broadcasts to `127.255.255.255` reach local sockets: resume (found by
/// discovery) works in this environment.
pub fn loopback_broadcast_works() -> bool {
    let probe = || -> std::io::Result<bool> {
        let rx = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))?;
        rx.set_read_timeout(Some(Duration::from_secs(1)))?;
        let port = rx.local_addr()?.port();
        let tx = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))?;
        tx.set_broadcast(true)?;
        tx.send_to(b"probe", (Ipv4Addr::new(127, 255, 255, 255), port))?;
        let mut buf = [0u8; 8];
        Ok(rx.recv_from(&mut buf).is_ok())
    };
    let ok = probe().unwrap_or(false);
    if !ok {
        eprintln!("skipping: loopback broadcast unavailable in this environment");
    }
    ok
}
