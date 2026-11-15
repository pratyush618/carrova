//! Pretend clipboard for [`FakePlatform`]: content kept in memory, shared
//! by clones; a write calls back every live watch.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use super::FakePlatform;
use super::watch::{Callback, locked};
use crate::{ClipContent, ClipboardWatch};

/// Content and live watches (by id).
#[derive(Debug, Default)]
pub(super) struct FakeClipboard {
    next: AtomicU64,
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    content: Option<ClipContent>,
    watches: Vec<(u64, Callback)>,
}

impl std::fmt::Debug for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("State")
            .field("watches", &self.watches.len())
            .finish_non_exhaustive()
    }
}

impl FakeClipboard {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl FakePlatform {
    pub(super) fn clip_read(&self) -> Option<ClipContent> {
        self.clipboard.lock().content.clone()
    }

    pub(super) fn clip_write(&self, content: &ClipContent) {
        let due: Vec<Callback> = {
            let mut state = self.clipboard.lock();
            state.content = Some(content.clone());
            state.watches.iter().map(|(_, c)| Arc::clone(c)).collect()
        };
        for changed in due {
            if let Some(changed) = locked(&changed).as_ref() {
                changed();
            }
        }
    }

    pub(super) fn clip_watch(&self, changed: Box<dyn Fn() + Send>) -> ClipboardWatch {
        let clipboard = Arc::clone(&self.clipboard);
        let id = clipboard.next.fetch_add(1, Ordering::SeqCst);
        let callback: Callback = Arc::new(Mutex::new(Some(changed)));
        clipboard.lock().watches.push((id, Arc::clone(&callback)));
        ClipboardWatch::new(move || {
            clipboard.lock().watches.retain(|(live, _)| *live != id);
            // Waits for a call in progress: none comes after the drop.
            locked(&callback).take();
        })
    }
}
