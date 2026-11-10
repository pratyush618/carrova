//! Why a target is not written ([`SkipReason`]) and the planner's verdict
//! for one target ([`Decision`]).

use std::fmt;

use crate::plan::Action;
use crate::plan::confine::{ProbeError, RootUnusable};

/// The planner's verdict for one target.
pub(super) enum Decision {
    /// Create / replace / keep-both / identical.
    Write(Action),
    /// Not written.
    Skip(SkipReason),
}

/// Why a target is not written. Every reason except the user's own
/// choices ([`Self::chosen_by_user`]) is also reported as a warning.
#[derive(Debug, Clone)]
pub(super) enum SkipReason {
    /// The target's root cannot be written below.
    Root(RootUnusable),
    RootUnavailable,
    UnknownHandler(String),
    DuplicateTarget,
    /// An ancestor (`root:rel`) is an incoming file or symlink.
    IncomingAncestor(String),
    /// An ancestor (`root:rel`) is a symlink on this machine.
    SymlinkAncestor(String),
    /// An ancestor (`root:rel`) exists and is not a directory.
    FileAncestor(String),
    AncestorUninspectable(String, ProbeError),
    /// The symlink target (verbatim) leaves the target root and home.
    SymlinkEscapes(String),
    /// The symlink target (verbatim) passes through an existing link.
    SymlinkThroughLink(String),
    SymlinkTargetUninspectable(String, ProbeError),
    TargetUninspectable(ProbeError),
    SpecialFile,
    DirOverNonDir,
    NonDirOverDir,
    /// Conflict rule `skip`.
    RuleSkip,
    /// Conflict rule `keep-both` on a directory.
    KeepBothDir,
    NoParent,
    NoKeepBothName,
}

impl SkipReason {
    /// The user's conflict rule chose this skip: no warning.
    pub(super) const fn chosen_by_user(&self) -> bool {
        matches!(self, Self::RuleSkip | Self::KeepBothDir)
    }
}

impl fmt::Display for SkipReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Root(r) => fmt::Display::fmt(r, f),
            Self::RootUnavailable => f.write_str("root unavailable"),
            Self::UnknownHandler(h) => write!(f, "unknown handler `{h}`"),
            Self::DuplicateTarget => f.write_str("another selected entry already writes this path"),
            Self::IncomingAncestor(at) => write!(f, "{at} is an incoming file or symlink"),
            Self::SymlinkAncestor(at) => {
                write!(f, "{at} is a symlink on this machine; never written through")
            }
            Self::FileAncestor(at) => write!(f, "{at} exists and is not a directory"),
            Self::AncestorUninspectable(at, e) => write!(f, "cannot inspect {at}: {e}"),
            Self::SymlinkEscapes(t) => {
                write!(f, "symlink target `{t}` points outside the target root and home")
            }
            Self::SymlinkThroughLink(t) => write!(
                f,
                "symlink target `{t}` goes through an existing link on this machine"
            ),
            Self::SymlinkTargetUninspectable(t, e) => {
                write!(f, "cannot inspect symlink target `{t}`: {e}")
            }
            Self::TargetUninspectable(e) => write!(f, "cannot inspect target: {e}"),
            Self::SpecialFile => {
                f.write_str("existing object is not a regular file, directory or symlink")
            }
            Self::DirOverNonDir => f.write_str(
                "a file or symlink exists where a directory is incoming; kept existing",
            ),
            Self::NonDirOverDir => f.write_str(
                "a directory exists where a file or symlink is incoming; a directory is never replaced",
            ),
            Self::RuleSkip => f.write_str("kept existing (rule: skip)"),
            Self::KeepBothDir => f.write_str(
                "kept existing directory mode (keep-both does not apply to directories)",
            ),
            Self::NoParent => f.write_str("target has no parent directory"),
            Self::NoKeepBothName => f.write_str("no free keep-both name; kept existing"),
        }
    }
}
