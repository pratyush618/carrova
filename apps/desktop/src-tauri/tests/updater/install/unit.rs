#![allow(clippy::unwrap_used)]

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use carrova_view::ipc::ErrorCode;
use serde_json::{Value, json};

use super::*;
use crate::testing::Recorder;

const MB: usize = 1_000_000;

/// A package of `chunks` that downloads at once (or never, when `stuck`).
#[derive(Default)]
struct FakePackage {
    chunks: Vec<usize>,
    stuck: bool,
    broken: bool,
    installed: AtomicBool,
}

impl FakePackage {
    fn of(chunks: &[usize]) -> Self {
        Self {
            chunks: chunks.to_vec(),
            ..Self::default()
        }
    }
}

impl Package for FakePackage {
    async fn fetch(
        &self,
        mut on_chunk: impl FnMut(usize, Option<u64>) + Send,
        on_downloaded: impl FnOnce() + Send,
    ) -> Result<Vec<u8>, CommandError> {
        if self.stuck {
            std::future::pending::<()>().await;
        }
        let total = self.chunks.iter().sum::<usize>() as u64;
        for &chunk in &self.chunks {
            on_chunk(chunk, Some(total));
        }
        on_downloaded();
        if self.broken {
            return Err(CommandError::new(ErrorCode::Network, "bad signature"));
        }
        Ok(vec![0; 4])
    }

    fn install(&self, _bytes: &[u8]) -> Result<(), CommandError> {
        self.installed.store(true, Ordering::SeqCst);
        Ok(())
    }
}

fn steps(out: &Recorder) -> Vec<Value> {
    out.of_kind("update")
        .into_iter()
        .map(|e| e["step"].clone())
        .collect()
}

fn names(out: &Recorder) -> Vec<String> {
    steps(out)
        .iter()
        .map(|s| s["step"].as_str().unwrap().to_owned())
        .collect()
}

fn signal(value: Signal) -> watch::Receiver<Signal> {
    watch::channel(value).1
}

fn run(
    package: &FakePackage,
    out: &Recorder,
    blocker: impl Fn() -> Option<UpdateBlocker> + Send,
    sig: watch::Receiver<Signal>,
) -> Result<(), CommandError> {
    tauri::async_runtime::block_on(install(package, out, blocker, sig))
}

#[test]
fn forwards_progress_then_the_check_then_installs() {
    let package = FakePackage::of(&[MB, MB]);
    let out = Recorder::default();
    run(&package, &out, || None, signal(Signal::Go)).unwrap();

    assert_eq!(
        names(&out),
        ["downloading", "downloading", "verifying", "installing"]
    );
    let last = &steps(&out)[1];
    assert_eq!(last["done"], json!(2 * MB));
    assert_eq!(last["percent"], json!(100));
    assert!(package.installed.load(Ordering::SeqCst));
}

#[test]
fn a_broken_package_installs_nothing() {
    let package = FakePackage {
        broken: true,
        ..FakePackage::of(&[MB])
    };
    let out = Recorder::default();
    let error = run(&package, &out, || None, signal(Signal::Go)).unwrap_err();

    assert_eq!(error.bridge().code, ErrorCode::Network);
    assert!(!names(&out).contains(&"installing".to_owned()));
    assert!(!package.installed.load(Ordering::SeqCst));
}

#[test]
fn cancel_stops_the_download() {
    let package = FakePackage {
        stuck: true,
        ..FakePackage::default()
    };
    let out = Recorder::default();
    let error = run(&package, &out, || None, signal(Signal::Cancel)).unwrap_err();

    assert_eq!(error.bridge().code, ErrorCode::Cancelled);
    assert!(!package.installed.load(Ordering::SeqCst));
}

#[test]
fn waits_for_a_running_move_then_installs() {
    let package = FakePackage::of(&[MB]);
    let out = Recorder::default();
    let looks = AtomicUsize::new(0);
    // The move ends after the second look.
    let blocker = || (looks.fetch_add(1, Ordering::SeqCst) < 2).then_some(UpdateBlocker::Move);
    run(&package, &out, blocker, signal(Signal::Go)).unwrap();

    assert_eq!(
        names(&out),
        ["downloading", "verifying", "waiting", "installing"],
        "one waiting step however often it looks"
    );
    assert_eq!(steps(&out)[2]["blocker"], json!("move"));
    assert!(package.installed.load(Ordering::SeqCst));
}

#[test]
fn install_anyway_skips_the_wait() {
    let package = FakePackage::of(&[MB]);
    let out = Recorder::default();
    run(
        &package,
        &out,
        || Some(UpdateBlocker::Call),
        signal(Signal::Anyway),
    )
    .unwrap();

    assert!(package.installed.load(Ordering::SeqCst));
}

#[test]
fn cancel_while_waiting_installs_nothing() {
    let package = FakePackage::of(&[MB]);
    let out = Recorder::default();
    let (tx, rx) = watch::channel(Signal::Go);
    let blocker = || {
        tx.send_replace(Signal::Cancel);
        Some(UpdateBlocker::Move)
    };
    let error = run(&package, &out, blocker, rx).unwrap_err();

    assert_eq!(error.bridge().code, ErrorCode::Cancelled);
    assert!(!package.installed.load(Ordering::SeqCst));
}
