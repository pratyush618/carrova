//! Sender: resume-`Hello` nonces already seen, so one `Hello` cannot be
//! replayed to hold a handshake slot.

use std::collections::{HashSet, VecDeque};
use std::sync::{Mutex, PoisonError};

use carrova_crypto::pairing::TagNonce;

/// Bounded memory of the nonces of recognised resume `Hello`s (the oldest
/// is forgotten first once `capacity` are remembered).
#[derive(Debug)]
pub(crate) struct ResumeNonces {
    capacity: usize,
    seen: Mutex<Seen>,
}

#[derive(Debug, Default)]
struct Seen {
    set: HashSet<TagNonce>,
    order: VecDeque<TagNonce>,
}

impl ResumeNonces {
    /// Remember at most `capacity` nonces.
    pub(crate) fn new(capacity: usize) -> Self {
        Self {
            capacity,
            seen: Mutex::new(Seen::default()),
        }
    }

    /// Record `nonce`: `true` if it was not seen before.
    pub(crate) fn first_use(&self, nonce: TagNonce) -> bool {
        let mut seen = self.seen.lock().unwrap_or_else(PoisonError::into_inner);
        if !seen.set.insert(nonce) {
            return false;
        }
        seen.order.push_back(nonce);
        if seen.order.len() > self.capacity
            && let Some(oldest) = seen.order.pop_front()
        {
            seen.set.remove(&oldest);
        }
        true
    }
}

#[cfg(test)]
#[path = "../tests/replay/unit.rs"]
mod tests;
