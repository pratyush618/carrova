//! What the receiver's OS family cannot hold: file names it rejects or
//! reserves, symlinks, case-distinct names. One data table, one row per
//! OS family with limits; every other OS takes [`POSIX`].

use std::fmt;

use carrova_platform::OsKind;

/// Limits of an OS family on what a restore may create.
pub(super) struct OsRules {
    /// Device names reserved in any directory, compared case-insensitively
    /// with the part of a name before its first dot, trailing spaces trimmed.
    reserved: &'static [&'static str],
    /// Characters no name may contain (besides control characters, when
    /// [`Self::no_control`]).
    forbidden: &'static [char],
    /// Characters no name may end with.
    forbidden_last: &'static [char],
    /// Control characters (`0x00`–`0x1f`) are rejected.
    no_control: bool,
    /// Names differing only in case are the same file.
    pub(super) case_insensitive: bool,
    /// Symlink entries are restored.
    pub(super) symlinks: bool,
}

/// Linux and macOS: any name a manifest may carry is valid.
// shortcut: macOS volumes are usually case-insensitive too, but not always;
// treat them case-sensitive until the platform reports it per filesystem.
const POSIX: OsRules = OsRules {
    reserved: &[],
    forbidden: &[],
    forbidden_last: &[],
    no_control: false,
    case_insensitive: false,
    symlinks: true,
};

/// Windows (NTFS / Win32 naming rules). `\` is a separator there: a name
/// holding one would be split into path components.
const WINDOWS: OsRules = OsRules {
    // Win32 also maps the superscript digits ¹ ² ³ to COM/LPT ports.
    reserved: &[
        "CON",
        "PRN",
        "AUX",
        "NUL",
        "CONIN$",
        "CONOUT$",
        "COM0",
        "COM1",
        "COM2",
        "COM3",
        "COM4",
        "COM5",
        "COM6",
        "COM7",
        "COM8",
        "COM9",
        "COM\u{b9}",
        "COM\u{b2}",
        "COM\u{b3}",
        "LPT0",
        "LPT1",
        "LPT2",
        "LPT3",
        "LPT4",
        "LPT5",
        "LPT6",
        "LPT7",
        "LPT8",
        "LPT9",
        "LPT\u{b9}",
        "LPT\u{b2}",
        "LPT\u{b3}",
    ],
    forbidden: &['<', '>', ':', '"', '|', '?', '*', '\\'],
    forbidden_last: &['.', ' '],
    no_control: true,
    case_insensitive: true,
    // shortcut: creating symlinks on Windows needs a privilege or developer
    // mode; skip them until the platform reports it can.
    symlinks: false,
};

impl OsRules {
    /// The rules of `os`.
    pub(super) const fn of(os: OsKind) -> &'static Self {
        match os {
            OsKind::Windows => &WINDOWS,
            OsKind::Linux | OsKind::MacOs => &POSIX,
        }
    }

    /// The first component of `rel` (`/`-separated) this OS rejects, and why.
    pub(super) fn bad_name(&self, rel: &str, os: OsKind) -> Option<BadName> {
        rel.split('/').find_map(|name| {
            self.problem(name).map(|problem| BadName {
                name: name.to_owned(),
                problem,
                os,
            })
        })
    }

    fn problem(&self, name: &str) -> Option<NameProblem> {
        if let Some(c) = name
            .chars()
            .find(|c| self.forbidden.contains(c) || (self.no_control && c.is_ascii_control()))
        {
            return Some(NameProblem::Char(c));
        }
        if let Some(c) = name
            .chars()
            .last()
            .filter(|c| self.forbidden_last.contains(c))
        {
            return Some(NameProblem::Last(c));
        }
        // Win32 drops trailing spaces before matching: `NUL .txt` is `NUL`.
        let stem = name.split('.').next().unwrap_or(name).trim_end_matches(' ');
        self.reserved
            .iter()
            .any(|r| r.eq_ignore_ascii_case(stem))
            .then_some(NameProblem::Reserved)
    }
}

/// A path component the receiver's OS cannot hold.
#[derive(Debug, Clone)]
pub(super) struct BadName {
    name: String,
    problem: NameProblem,
    os: OsKind,
}

#[derive(Debug, Clone, Copy)]
enum NameProblem {
    Char(char),
    Last(char),
    Reserved,
}

impl fmt::Display for BadName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (name, os) = (&self.name, self.os);
        match self.problem {
            NameProblem::Char(c) => {
                write!(f, "name `{name}`: character {c:?} is not allowed on {os}")
            }
            NameProblem::Last(c) => write!(f, "name `{name}` ends with {c:?}, not allowed on {os}"),
            NameProblem::Reserved => write!(f, "name `{name}` is reserved on {os}"),
        }
    }
}
