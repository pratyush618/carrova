#![allow(clippy::unwrap_used)]

use std::time::Duration;

use carrova_view::ipc::ErrorCode;
use tauri::async_runtime::block_on;

use super::*;

fn counter() -> Worker<u32> {
    block_on(Worker::open("test", || Ok::<_, CommandError>((0_u32, ()))))
        .unwrap()
        .0
}

#[test]
fn jobs_run_in_order_on_one_owned_object() {
    let worker = counter();
    block_on(worker.call(|n| *n += 1)).unwrap();
    block_on(worker.clone().call(|n| *n *= 10)).unwrap();
    assert_eq!(block_on(worker.call(|n| *n)).unwrap(), 10);
}

#[test]
fn jobs_run_on_the_worker_thread() {
    let worker = counter();
    let name = block_on(worker.call(|_| thread::current().name().map(str::to_owned))).unwrap();
    assert_eq!(name.as_deref(), Some("carrova-test"));
}

#[test]
fn a_failed_open_returns_its_error() {
    let err = block_on(Worker::<u32>::open("test", || {
        Err::<(u32, ()), _>(CommandError::no_session("nope"))
    }))
    .unwrap_err();
    assert_eq!(err.bridge().code, ErrorCode::Session);
}

#[test]
fn a_panicking_job_reports_the_worker_gone() {
    let worker = counter();
    let err = block_on(worker.call(|_| -> u32 { std::panic::resume_unwind(Box::new("boom")) }))
        .unwrap_err();
    assert_eq!(err.bridge().code, ErrorCode::Internal);
    let err = block_on(worker.call(|n| *n)).unwrap_err();
    assert_eq!(err.bridge().code, ErrorCode::Internal);
}

#[test]
fn busy_while_a_job_runs() {
    let worker = counter();
    let (started, wait) = mpsc::channel();
    let running = worker.clone();
    let job = thread::spawn(move || {
        block_on(running.call(move |_| {
            started.send(()).unwrap();
            thread::sleep(Duration::from_millis(100));
        }))
    });
    wait.recv().unwrap();
    assert!(worker.is_busy());
    job.join().unwrap().unwrap();
}

#[test]
fn idle_as_soon_as_an_open_or_a_call_has_answered() {
    // A command right after another (dismiss after scan) must not see the
    // worker busy; repeated because the old window was microseconds wide.
    for _ in 0..2000 {
        let worker = counter();
        assert!(!worker.is_busy(), "busy after open answered");
        block_on(worker.call(|n| *n += 1)).unwrap();
        assert!(!worker.is_busy(), "busy after a call answered");
    }
}
