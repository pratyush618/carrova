use std::cell::Cell;
use std::path::Path;
use std::time::{Duration, Instant};

use carrova_platform::fake::FakePlatform;

use super::Disk;
use crate::cancel::Cancel;
use crate::error::CoreError;
use crate::session::NullSink;
use crate::transfer_control::TransferControl;

#[test]
fn really_full_disk_with_guard_lifted_retries_without_spinning() {
    let platform = FakePlatform::with_home("/h");
    let cancel = Cancel::default();
    let control = TransferControl::default();
    control.ignore_free_space();
    let disk = Disk::new(
        &platform,
        Path::new("/h/stage"),
        &NullSink,
        Path::new("/h"),
        &cancel,
        &control,
    );

    let calls = Cell::new(0_u32);
    let started = Instant::now();
    let result = disk.wait(|| {
        calls.set(calls.get() + 1);
        if started.elapsed() < Duration::from_millis(300) {
            Err(CoreError::DiskFull {
                needed: 1,
                available: 0,
            })
        } else {
            Ok(())
        }
    });

    assert!(result.is_ok());
    assert!(
        calls.get() <= 10,
        "retried {} times in 300 ms: hot loop",
        calls.get()
    );
}
