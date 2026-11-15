//! File offer wire types.

use serde::{Deserialize, Serialize};

/// A file one device offers another.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileOffer {
    /// Sender-chosen offer id.
    pub id: u64,
    /// Name as the sender knows it; run through `safe_file_name` before use.
    pub name: String,
    /// Size in bytes.
    pub size: u64,
    /// Media type, if known.
    pub mime: Option<String>,
}

/// The receiver's reply to a [`FileOffer`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OfferAnswer {
    /// Send the file, starting at this byte offset.
    Accept {
        /// First byte the receiver still needs.
        resume_from: u64,
    },
    /// The user said no.
    Decline,
    /// The offer timed out unanswered.
    Expired,
}
