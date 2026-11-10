use super::*;

#[test]
fn sleep_ends_early_once_cancelled() {
    let cancel = Cancel::default();
    let other = cancel.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(100));
        other.cancel();
    });
    let started = Instant::now();
    assert!(matches!(
        cancel.sleep(Duration::from_secs(30)),
        Err(CoreError::Cancelled)
    ));
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[test]
fn the_transport_sees_the_same_flag() {
    let cancel = Cancel::default();
    let signal = cancel.signal();
    assert!(!signal.is_cancelled());
    cancel.cancel();
    assert!(signal.is_cancelled());
}
