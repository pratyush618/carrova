#![allow(clippy::unwrap_used)]

use std::collections::VecDeque;
use std::io::{self, Cursor};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use serde_json::Value;
use tempfile::TempDir;

use super::*;
use crate::testing::Recorder;

const SIZE: usize = 300 * 1024;

/// The APK's bytes: every byte tells where it is.
fn content() -> Vec<u8> {
    (0..SIZE).map(|i| (i % 251) as u8).collect()
}

/// Serves `content()`, from the offset asked (unless it ignores ranges).
#[derive(Default)]
struct FakeSource {
    ignores_range: bool,
    /// Fail the first read after this many bytes of the body.
    breaks_after: Mutex<Option<usize>>,
    /// Wi-Fi drops as the read fails.
    drops: Option<Arc<AtomicBool>>,
    /// Claim this size but send less.
    short: bool,
    asked: Mutex<Vec<u64>>,
}

impl FakeSource {
    fn breaking(after: usize) -> Self {
        Self {
            breaks_after: Mutex::new(Some(after)),
            ..Self::default()
        }
    }
}

/// A reader that fails after its bytes, taking Wi-Fi with it.
struct Breaking(Cursor<Vec<u8>>, Option<Arc<AtomicBool>>);

impl Read for Breaking {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self.0.read(buf)? {
            0 => {
                if let Some(wifi) = &self.1 {
                    wifi.store(false, Ordering::SeqCst);
                }
                Err(io::ErrorKind::ConnectionReset.into())
            }
            n => Ok(n),
        }
    }
}

impl Source for FakeSource {
    fn open(&self, _url: &str, offset: u64) -> Result<Body, CommandError> {
        self.asked.lock().unwrap().push(offset);
        let start = if self.ignores_range { 0 } else { offset };
        let mut rest = content().split_off(start as usize);
        let total = Some(SIZE as u64);
        if self.short {
            rest.truncate(rest.len() / 2);
        }
        let reader: Box<dyn Read + Send> = match self.breaks_after.lock().unwrap().take() {
            Some(n) => {
                rest.truncate(n);
                Box::new(Breaking(Cursor::new(rest), self.drops.clone()))
            }
            None => Box::new(Cursor::new(rest)),
        };
        Ok(Body {
            reader,
            start,
            total,
        })
    }
}

/// A phone on Wi-Fi or not, with taps queued by the test.
struct FakePhone {
    wifi: Arc<AtomicBool>,
    taps: Mutex<VecDeque<UpdateTap>>,
    shown: Mutex<Vec<Option<UpdateStage>>>,
}

impl FakePhone {
    fn new(wifi: bool) -> Self {
        Self {
            wifi: Arc::new(AtomicBool::new(wifi)),
            taps: Mutex::default(),
            shown: Mutex::default(),
        }
    }
}

impl Phone for FakePhone {
    fn on_wifi(&self) -> bool {
        self.wifi.load(Ordering::SeqCst)
    }

    fn show(&self, notice: Option<&UpdateNotice>) {
        self.shown.lock().unwrap().push(notice.map(|n| n.stage));
    }

    fn tap(&self) -> Option<UpdateTap> {
        self.taps.lock().unwrap().pop_front()
    }
}

struct Run {
    dir: TempDir,
    out: Recorder,
    control: Arc<Control>,
}

impl Run {
    fn new() -> Self {
        Self {
            dir: TempDir::new().unwrap(),
            out: Recorder::default(),
            control: Arc::default(),
        }
    }

    fn part(&self) -> PathBuf {
        self.dir.path().join("Carrova_0.4.0_android-arm64.apk.part")
    }

    fn go(
        &self,
        source: &FakeSource,
        phone: &FakePhone,
        mobile_data: bool,
    ) -> Result<(), CommandError> {
        let part = self.part();
        let job = Job {
            url: "https://github.com/x.apk",
            part: &part,
            version: "0.4.0",
            mobile_data,
        };
        download(source, phone, &self.out, &self.control, job)
    }

    fn steps(&self) -> Vec<Value> {
        self.out
            .of_kind("update")
            .into_iter()
            .map(|e| e["step"].clone())
            .collect()
    }

    fn names(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .steps()
            .iter()
            .map(|s| s["step"].as_str().unwrap().to_owned())
            .collect();
        names.dedup();
        names
    }
}

#[test]
fn downloads_the_whole_file_with_progress() {
    let run = Run::new();
    run.go(&FakeSource::default(), &FakePhone::new(true), false)
        .unwrap();
    assert_eq!(std::fs::read(run.part()).unwrap(), content());
    assert_eq!(run.names(), ["downloading"]);
    let last = run.steps().pop().unwrap();
    assert_eq!(last["done"], SIZE);
    assert_eq!(last["percent"], 100);
}

#[test]
fn resumes_from_the_partial_file_with_a_range() {
    let run = Run::new();
    std::fs::write(run.part(), &content()[..1000]).unwrap();
    let source = FakeSource::default();
    run.go(&source, &FakePhone::new(true), false).unwrap();
    assert_eq!(*source.asked.lock().unwrap(), [1000]);
    assert_eq!(std::fs::read(run.part()).unwrap(), content());
    assert_eq!(run.steps()[0]["done"], 1000);
}

#[test]
fn a_server_ignoring_the_range_starts_over() {
    let run = Run::new();
    std::fs::write(run.part(), &content()[..1000]).unwrap();
    let source = FakeSource {
        ignores_range: true,
        ..FakeSource::default()
    };
    run.go(&source, &FakePhone::new(true), false).unwrap();
    assert_eq!(std::fs::read(run.part()).unwrap(), content());
}

#[test]
fn off_wifi_it_waits_until_mobile_data_is_allowed() {
    let run = Run::new();
    let phone = FakePhone::new(false);
    let control = Arc::clone(&run.control);
    let allow = thread::spawn(move || {
        thread::sleep(Duration::from_millis(100));
        control.update(|wish| wish.mobile_data = true);
    });
    run.go(&FakeSource::default(), &phone, false).unwrap();
    allow.join().unwrap();
    assert_eq!(run.names(), ["waiting", "downloading"]);
    assert_eq!(run.steps()[0]["blocker"], "wifi");
    assert_eq!(
        phone.shown.lock().unwrap()[0],
        Some(UpdateStage::WaitingForWifi)
    );
}

#[test]
fn mobile_data_allowed_in_settings_downloads_off_wifi() {
    let run = Run::new();
    run.go(&FakeSource::default(), &FakePhone::new(false), true)
        .unwrap();
    assert_eq!(run.names(), ["downloading"]);
}

#[test]
fn wifi_dropping_mid_download_waits_then_resumes_where_it_stopped() {
    let run = Run::new();
    let phone = FakePhone::new(true);
    let source = FakeSource {
        drops: Some(Arc::clone(&phone.wifi)),
        ..FakeSource::breaking(1000)
    };
    let wifi = Arc::clone(&phone.wifi);
    let back = thread::spawn(move || {
        while wifi.load(Ordering::SeqCst) {
            thread::yield_now();
        }
        thread::sleep(Duration::from_millis(100));
        wifi.store(true, Ordering::SeqCst);
    });
    run.go(&source, &phone, false).unwrap();
    back.join().unwrap();
    assert_eq!(run.names(), ["downloading", "waiting", "downloading"]);
    assert_eq!(*source.asked.lock().unwrap(), [0, 1000]);
    assert_eq!(std::fs::read(run.part()).unwrap(), content());
}

#[test]
fn a_paused_download_holds_until_resumed() {
    let run = Run::new();
    run.control.update(|wish| wish.paused = true);
    let control = Arc::clone(&run.control);
    let resume = thread::spawn(move || {
        thread::sleep(Duration::from_millis(100));
        control.update(|wish| wish.paused = false);
    });
    run.go(&FakeSource::default(), &FakePhone::new(true), false)
        .unwrap();
    resume.join().unwrap();
    assert_eq!(run.names(), ["paused", "downloading"]);
}

#[test]
fn a_notification_tap_pauses_and_resumes() {
    let run = Run::new();
    let phone = FakePhone::new(true);
    phone
        .taps
        .lock()
        .unwrap()
        .extend([UpdateTap::Pause, UpdateTap::Resume]);
    run.go(&FakeSource::default(), &phone, false).unwrap();
    assert_eq!(run.names(), ["paused", "downloading"]);
    assert_eq!(std::fs::read(run.part()).unwrap(), content());
}

#[test]
fn cancelling_deletes_the_partial_file() {
    let run = Run::new();
    std::fs::write(run.part(), b"half").unwrap();
    run.control.update(|wish| wish.cancelled = true);
    let err = run
        .go(&FakeSource::default(), &FakePhone::new(true), false)
        .unwrap_err();
    assert_eq!(err.bridge().code, ErrorCode::Cancelled);
    assert!(!run.part().exists());
}

#[test]
fn a_broken_connection_fails_and_keeps_what_arrived() {
    let run = Run::new();
    let source = FakeSource::breaking(1000);
    let err = run.go(&source, &FakePhone::new(true), false).unwrap_err();
    assert_eq!(err.bridge().code, ErrorCode::Network);
    assert_eq!(std::fs::read(run.part()).unwrap(), &content()[..1000]);
}

#[test]
fn a_body_shorter_than_its_size_fails_and_can_resume() {
    let run = Run::new();
    let source = FakeSource {
        short: true,
        ..FakeSource::default()
    };
    let err = run.go(&source, &FakePhone::new(true), false).unwrap_err();
    assert_eq!(err.bridge().code, ErrorCode::Network);
    assert_eq!(std::fs::read(run.part()).unwrap().len(), SIZE / 2);
}

#[test]
fn the_notification_goes_when_the_download_ends() {
    let run = Run::new();
    let phone = FakePhone::new(true);
    run.go(&FakeSource::default(), &phone, false).unwrap();
    assert_eq!(phone.shown.lock().unwrap().last(), Some(&None));
}
