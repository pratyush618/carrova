//! [`FileLock`]: the guard returned by [`crate::Platform::lock_exclusive`].

use std::fs::File;

/// An exclusive advisory lock on a file, held until the guard is dropped
/// (closing the file releases it). Advisory: it only excludes other
/// lockers (other Carrova processes, or other threads of this one), never
/// plain readers or writers.
#[derive(Debug)]
#[must_use = "the lock is released as soon as the guard is dropped"]
pub struct FileLock {
    _file: File,
}

impl FileLock {
    /// Guard over `file`, which already holds the lock.
    pub(crate) fn held(file: File) -> Self {
        Self { _file: file }
    }
}

/// Holds the OS open to multicast traffic (Android filters it unless an
/// app asks) until the guard is dropped.
#[derive(Debug)]
#[must_use = "multicast may be filtered again as soon as the guard is dropped"]
pub struct MulticastGuard(());

impl MulticastGuard {
    /// Guard for an OS that never filters multicast.
    pub(crate) fn unfiltered() -> Self {
        Self(())
    }
}
