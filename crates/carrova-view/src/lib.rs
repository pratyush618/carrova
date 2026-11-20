//! Pure presentation models of engine results: what to show and how much
//! of it. No I/O (except reading the saved [`ipc::Settings`]) and no
//! escaping: front-ends (CLI text, desktop UI) render them and escape every
//! peer-provided string themselves.

/// `Debug` that prints only the type's name, for view-models carrying a
/// phone's private content (text, names, numbers): it never reaches a log.
macro_rules! private_debug {
    ($($t:ty),* $(,)?) => {$(
        impl std::fmt::Debug for $t {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(concat!(stringify!($t), "(..)"))
            }
        }
    )*};
}

pub mod devices;
pub mod ecosystem;
pub mod ipc;
pub mod lan;
pub mod notice;
pub mod offer;
pub mod passphrase;
pub mod phone;
pub mod plan;
pub mod request;
pub mod review;
pub mod selection;
pub mod tray;

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
