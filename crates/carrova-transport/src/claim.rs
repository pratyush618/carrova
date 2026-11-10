//! Sender: every pairing code is single-use. The first receiver whose PAKE
//! message arrives claims the code; no later attempt — in this or any
//! later accept call on the same listener — can use it again.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use carrova_crypto::pairing::PairingCode;

/// One code's single use, shared by the attempts racing for it.
#[derive(Debug, Default)]
pub(crate) struct CodeClaim(AtomicBool);

impl CodeClaim {
    /// Claim the code for one attempt: `true` for the first caller only.
    pub(crate) fn take(&self) -> bool {
        !self.0.swap(true, Ordering::AcqRel)
    }

    /// Whether an attempt has claimed the code (it is spent, or in use).
    pub(crate) fn is_taken(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

/// The claims of every code a listener has offered.
#[derive(Debug, Default)]
pub(crate) struct CodeBook(Vec<(PairingCode, Arc<CodeClaim>)>);

impl CodeBook {
    /// The claim of `code` (created unclaimed on first use).
    pub(crate) fn claim(&mut self, code: &PairingCode) -> Arc<CodeClaim> {
        if let Some((_, claim)) = self.0.iter().find(|(c, _)| c == code) {
            return Arc::clone(claim);
        }
        let claim = Arc::new(CodeClaim::default());
        self.0.push((code.clone(), Arc::clone(&claim)));
        claim
    }
}

#[cfg(test)]
#[path = "../tests/claim/unit.rs"]
mod tests;
