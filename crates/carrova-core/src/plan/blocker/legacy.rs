//! Reader of the frozen text form plans stored blockers in before they
//! were tagged objects. The pieces below are that format's, not the live
//! display texts: they must never change, even if [`Blocker`]'s
//! `Display` does.

use carrova_platform::SemanticRoot;

use super::Blocker;
use crate::space::{FsLocation, SpaceShortfall};

const SENSITIVE: &str = "` is sensitive (";
const SENSITIVE_END: &str = "): confirm it explicitly to import it, or deselect it";
const RUNNING_END: &str =
    " is running: close it before applying, or override the requires-closed check";
/// Between a running app's name and its quoted id: `Name (`id`)`.
const NAME_ID: &str = " (`";
const FREE_SPACE: &str = "not enough free space: ";
const NEEDED_ON: &str = " bytes needed on the filesystem of ";
const MARGIN: &str = " (incl. margin), ";
const AVAILABLE: &str = " available";
const STATE_DIR: &str = "the state dir";
const STATE_DIR_AND: &str = "the state dir and ";
const ROOT_SEPARATOR: &str = ", ";

/// The blocker a stored text describes; `None` if it is not one.
pub(super) fn parse(s: &str) -> Option<Blocker> {
    if let Some((item, reason)) = s
        .strip_prefix('`')
        .and_then(|q| q.strip_suffix(SENSITIVE_END))
        .and_then(|q| q.split_once(SENSITIVE))
    {
        return Some(Blocker::UnconfirmedSensitive {
            item: item.into(),
            reason: reason.to_owned(),
        });
    }
    if let Some(running) = s.strip_suffix(RUNNING_END) {
        return parse_running(running);
    }
    s.strip_prefix(FREE_SPACE)
        .and_then(parse_shortfall)
        .map(Blocker::FreeSpace)
}

/// The app of a running-app text (before [`RUNNING_END`]): `Name
/// (`id`)`, or the older `` `id` `` (the id doubles as the name).
fn parse_running(text: &str) -> Option<Blocker> {
    let (name, app) = match text.strip_suffix("`)").and_then(|t| t.rsplit_once(NAME_ID)) {
        Some((name, app)) => (name, app),
        None => {
            let app = text.strip_prefix('`')?.strip_suffix('`')?;
            (app, app)
        }
    };
    (!app.is_empty() && !app.contains('`')).then(|| Blocker::RunningApp {
        app: app.to_owned(),
        name: name.to_owned(),
    })
}

/// `N bytes needed on the filesystem of LOCATION (incl. margin), M
/// available`.
fn parse_shortfall(s: &str) -> Option<SpaceShortfall> {
    let (needed, rest) = s.split_once(NEEDED_ON)?;
    let (location, available) = rest.strip_suffix(AVAILABLE)?.rsplit_once(MARGIN)?;
    Some(SpaceShortfall {
        needed: needed.parse().ok()?,
        available: available.parse().ok()?,
        location: parse_location(location)?,
    })
}

/// `the state dir`, `the state dir and user_config, home` or `documents`.
fn parse_location(s: &str) -> Option<FsLocation> {
    let (state_dir, roots) = match s.strip_prefix(STATE_DIR_AND) {
        Some(roots) => (true, roots),
        None if s == STATE_DIR => (true, ""),
        None => (false, s),
    };
    let roots = if roots.is_empty() {
        Vec::new()
    } else {
        roots
            .split(ROOT_SEPARATOR)
            .map(|name| SemanticRoot::ALL.into_iter().find(|r| r.as_str() == name))
            .collect::<Option<_>>()?
    };
    Some(FsLocation { state_dir, roots })
}
