//! Permission policy for restored objects — the single source of truth used
//! by both [`crate::plan`] (authority: computes [`crate::PlannedAction::mode`])
//! and apply (defence in depth: re-applies the same policy to what it sets).
//!
//! - Only permission bits (`0o777`) are restored: setuid, setgid and sticky
//!   are never set.
//! - World-write is never restored.
//! - Group-write is dropped unless the user opts in with
//!   [`crate::PlanOptions::keep_group_write`] (e.g. a umask-002 setup).
//! - An existing directory is only ever tightened:
//!   `final = existing & restored`.

use crate::plan::PlanOptions;

/// Group-write bit.
const GROUP_WRITE: u32 = 0o020;
/// World-write bit.
const WORLD_WRITE: u32 = 0o002;
/// Setuid bit.
const SETUID: u32 = 0o4000;
/// Setgid bit.
const SETGID: u32 = 0o2000;
/// Sticky bit.
const STICKY: u32 = 0o1000;
/// Permission bits.
const PERMS: u32 = 0o777;

/// Which archive mode bits are restored (see the module docs).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ModePolicy {
    /// Keep group-write bits from the archive.
    pub keep_group_write: bool,
}

impl ModePolicy {
    /// The policy a plan was computed with.
    #[must_use]
    pub const fn of(options: &PlanOptions) -> Self {
        Self {
            keep_group_write: options.keep_group_write,
        }
    }

    /// Bits of an incoming mode that may be restored.
    const fn mask(self) -> u32 {
        if self.keep_group_write {
            PERMS & !WORLD_WRITE
        } else {
            PERMS & !(WORLD_WRITE | GROUP_WRITE)
        }
    }

    /// Final mode for an object with archive mode `incoming`; `existing_dir`
    /// is the mode of a directory already at the target (only tightened).
    #[must_use]
    pub const fn final_mode(self, incoming: u32, existing_dir: Option<u32>) -> u32 {
        let restored = incoming & self.mask();
        match existing_dir {
            Some(existing) => restored & existing & PERMS,
            None => restored,
        }
    }
}

/// Classes of archive mode bits that were not restored, accumulated over
/// any number of paths (for one summarised plan warning per item).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DroppedBits {
    /// Paths whose final mode differs from the archive's.
    pub paths: usize,
    /// Union of `archive & !final` over those paths.
    bits: u32,
}

impl DroppedBits {
    /// Record one path whose archive mode `from` became `to`.
    pub fn add(&mut self, from: u32, to: u32) {
        let dropped = from & !to;
        if dropped != 0 {
            self.paths += 1;
            self.bits |= dropped;
        }
    }

    /// Only group-write was dropped (restorable with
    /// [`ModePolicy::keep_group_write`]).
    #[must_use]
    pub const fn group_write_only(&self) -> bool {
        self.bits == GROUP_WRITE
    }

    /// Human-readable classes, e.g. `["group-write", "setuid"]`.
    #[must_use]
    pub fn classes(&self) -> Vec<&'static str> {
        let tightened = self.bits & PERMS & !(GROUP_WRITE | WORLD_WRITE);
        [
            (GROUP_WRITE, "group-write"),
            (WORLD_WRITE, "world-write"),
            (SETUID, "setuid"),
            (SETGID, "setgid"),
            (STICKY, "sticky"),
            (tightened, "other bits (existing directory kept)"),
        ]
        .into_iter()
        .filter(|(bit, _)| self.bits & bit != 0)
        .map(|(_, name)| name)
        .collect()
    }
}

#[cfg(test)]
#[path = "../tests/mode/unit.rs"]
mod tests;
