//! Sender: which incoming connections may start a handshake.
//!
//! An unvalidated source address first gets a QUIC Retry (a spoofed source
//! never sees the token, so it never reaches a slot). A validated one gets
//! a handshake slot if one is free and its address holds fewer than
//! [`MAX_PENDING_PER_ADDRESS`]; otherwise it is refused at once rather
//! than queued.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex, PoisonError};

use crate::tuning::MAX_PENDING_PER_ADDRESS;

/// What to do with one incoming connection.
#[derive(Debug)]
pub(crate) enum Admission {
    /// Prove the source address first (QUIC Retry).
    Validate,
    /// Turn it away now.
    Refuse(Refusal),
    /// Run the handshake while holding this slot.
    Admit(Slot),
}

/// Why an incoming connection was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Refusal {
    /// Every handshake slot is busy.
    Full,
    /// Its address already holds its share of slots.
    AddressBusy,
}

/// Pending-handshake bookkeeping shared by the slots it hands out.
#[derive(Debug)]
pub(crate) struct HandshakeSlots {
    capacity: usize,
    pending: Arc<Mutex<HashMap<IpAddr, usize>>>,
}

impl HandshakeSlots {
    /// At most `capacity` handshakes at once.
    pub(crate) fn new(capacity: usize) -> Self {
        Self {
            capacity,
            pending: Arc::default(),
        }
    }

    /// Decide on `incoming`.
    pub(crate) fn admit(&self, incoming: &quinn::Incoming) -> Admission {
        if incoming.remote_address_validated() {
            self.reserve(incoming.remote_address().ip())
        } else {
            Admission::Validate
        }
    }

    /// A slot for a validated connection from `ip`, if allowed.
    fn reserve(&self, ip: IpAddr) -> Admission {
        let mut pending = self.pending.lock().unwrap_or_else(PoisonError::into_inner);
        if pending.values().sum::<usize>() >= self.capacity {
            return Admission::Refuse(Refusal::Full);
        }
        let held = pending.entry(ip).or_default();
        if *held >= MAX_PENDING_PER_ADDRESS {
            return Admission::Refuse(Refusal::AddressBusy);
        }
        *held += 1;
        Admission::Admit(Slot {
            ip,
            pending: Arc::clone(&self.pending),
        })
    }
}

/// One held handshake slot; released when dropped.
#[derive(Debug)]
pub(crate) struct Slot {
    ip: IpAddr,
    pending: Arc<Mutex<HashMap<IpAddr, usize>>>,
}

impl Drop for Slot {
    fn drop(&mut self) {
        let mut pending = self.pending.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(held) = pending.get_mut(&self.ip) {
            *held -= 1;
            if *held == 0 {
                pending.remove(&self.ip);
            }
        }
    }
}

#[cfg(test)]
#[path = "../tests/admission/unit.rs"]
mod tests;
