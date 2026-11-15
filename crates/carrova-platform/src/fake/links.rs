//! Pretend background service for [`FakePlatform`]: every
//! [`Platform::keep_running`](crate::Platform::keep_running) is recorded,
//! and linking is paused only when a test says so.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, PoisonError};

use super::FakePlatform;
use crate::LinkNotice;

/// Shared by clones of one fake, like the hooks.
#[derive(Debug, Default)]
pub(super) struct FakeLinks {
    kept: Mutex<Vec<Option<LinkNotice>>>,
    paused: AtomicBool,
}

impl FakeLinks {
    pub(super) fn keep_running(&self, notice: Option<&LinkNotice>) {
        self.kept
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(notice.cloned());
    }

    pub(super) fn paused(&self) -> bool {
        self.paused.load(Ordering::SeqCst)
    }
}

impl FakePlatform {
    /// Every notice [`Platform::keep_running`](crate::Platform::keep_running)
    /// was given, oldest first (`None`: let it stop).
    #[must_use]
    pub fn kept_running(&self) -> Vec<Option<LinkNotice>> {
        self.links
            .kept
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Pretend the user paused (`true`) or resumed linking
    /// ([`Platform::links_paused`](crate::Platform::links_paused)).
    pub fn set_links_paused(&self, paused: bool) {
        self.links.paused.store(paused, Ordering::SeqCst);
    }
}
