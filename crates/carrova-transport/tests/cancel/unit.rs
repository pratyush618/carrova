use std::time::Duration;

use super::*;

#[test]
fn a_pending_call_ends_once_the_flag_is_set() {
    let flag = Arc::new(AtomicBool::new(false));
    let signal: Arc<dyn CancelSignal> = flag.clone();
    let watch = Watch::new(Some(&signal));
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(100));
        flag.store(true, Ordering::Release);
    });
    let result: Result<()> = watch.block_on(std::future::pending());
    assert!(matches!(result, Err(TransportError::Cancelled)));
}

#[test]
fn a_finished_call_wins_over_a_cancel() {
    let signal: Arc<dyn CancelSignal> = Arc::new(AtomicBool::new(true));
    let watch = Watch::new(Some(&signal));
    assert_eq!(watch.block_on(async { Ok(7) }).unwrap(), 7);
}

#[test]
fn without_a_signal_nothing_is_cancelled() {
    let watch = Watch::default();
    assert!(!watch.is_cancelled());
    assert_eq!(watch.block_on(async { Ok(1) }).unwrap(), 1);
}
