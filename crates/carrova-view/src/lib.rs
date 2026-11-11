//! Pure presentation models of engine results: what to show and how much
//! of it. No I/O and no escaping: front-ends (CLI text, desktop UI) render them
//! and escape every peer-provided string themselves.

pub mod ipc;
pub mod lan;
pub mod notice;
pub mod offer;
pub mod passphrase;
pub mod plan;
pub mod review;
pub mod selection;

/// How much of a long listing to show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Detail {
    /// Attention lines individually, the rest summarised.
    Summary,
    /// Everything individually.
    Full,
}

impl Detail {
    /// `-v` lists everything; default output summarises.
    #[must_use]
    pub fn from_verbosity(verbose: u8) -> Self {
        if verbose > 0 {
            Self::Full
        } else {
            Self::Summary
        }
    }
}
