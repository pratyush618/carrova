//! This device's presence on the LAN outside sessions, as far as the
//! user's [`Visibility`] allows (ADR 0001 §5): known devices recognise it
//! by tags only they can check; everyone else sees its name and OS only
//! while the user shows it to everyone.
//!
//! The user's choice is kept in `presence.json` in the state dir
//! ([`load_visibility`], [`save_visibility`]), so the CLI and the desktop
//! app share one value.

use std::sync::Arc;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use carrova_platform::{DirWatch, Platform};
use carrova_transport::{KnownTags, MAX_KNOWN_TAGS, PresenceAdvert, PresenceBeacon};
use serde::{Deserialize, Serialize};

use crate::devices::{DEVICES_DIR, DeviceRecord, KnownDevices};
use crate::engine::Engine;
use crate::error::{CoreError, EncodeError, Result};
use crate::lan::{clean_device_name, known_by_recent_use};
use crate::store::{ensure_dir, read_file, write_atomic};
use crate::time::now_unix;

/// How often a running [`Presence`] re-announces itself under a new mDNS
/// instance with fresh tags, so its adverts cannot be linked over a long
/// time by `kt` or instance name (each restart costs one mDNS announce).
pub const REFRESH: Duration = Duration::from_secs(10 * 60);

/// How often a running [`Presence`] checks the saved visibility and known
/// devices where the platform cannot watch the state dir for changes (the
/// watch refused or ended, e.g. no inotify watch to spare), so a change
/// made elsewhere (the CLI) applies within this long. Watched, it applies
/// at once and nothing is checked on a timer.
pub const POLL: Duration = Duration::from_secs(5);

/// How long "everyone" lasts when the user picks it
/// ([`Visibility::everyone_from`]).
pub const EVERYONE_FOR: Duration = Duration::from_secs(10 * 60);

/// The user's [`Visibility`], in the state dir.
pub const VISIBILITY_FILE: &str = "presence.json";

/// Who may find this device nearby.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Visibility {
    /// Nobody: no advert.
    NoOne,
    /// Known devices only: tags, no name or OS.
    #[default]
    Known,
    /// Everyone, until the wall-clock deadline `until` (Unix seconds); then
    /// [`Self::Known`].
    Everyone {
        /// Deadline, Unix seconds.
        until: u64,
    },
}

impl Visibility {
    /// The visibility in force at `now`: [`Self::Everyone`] past its
    /// deadline is [`Self::Known`].
    #[must_use]
    pub const fn at(self, now: u64) -> Self {
        match self {
            Self::Everyone { until } if now >= until => Self::Known,
            other => other,
        }
    }

    /// Everyone, for [`EVERYONE_FOR`] from `now` (Unix seconds).
    #[must_use]
    pub const fn everyone_from(now: u64) -> Self {
        Self::Everyone {
            until: now.saturating_add(EVERYONE_FOR.as_secs()),
        }
    }
}

/// The visibility the user chose, as in force at `now`: [`Visibility::Known`]
/// when none was saved, when the file is from another version or
/// unreadable as one (logged; the default is never a reason to fail), or
/// when "everyone" has run out.
///
/// # Errors
///
/// The state dir or the file cannot be read.
pub fn load_visibility(platform: &dyn Platform, now: u64) -> Result<Visibility> {
    let Some(bytes) = read_file(platform, &platform.state_dir()?.join(VISIBILITY_FILE))? else {
        return Ok(Visibility::default());
    };
    let saved = serde_json::from_slice::<Visibility>(&bytes)
        .inspect_err(|e| tracing::warn!(error = %e, "saved visibility unreadable: known devices"))
        .unwrap_or_default();
    Ok(saved.at(now))
}

/// Keep `visibility` as the user's choice (atomically, mode `600`).
///
/// # Errors
///
/// The state dir or the file cannot be written.
pub fn save_visibility(platform: &dyn Platform, visibility: Visibility) -> Result<()> {
    let dir = platform.state_dir()?;
    ensure_dir(platform, &dir, 0o700)?;
    let bytes = serde_json::to_vec(&visibility).map_err(|e| CoreError::Encode {
        what: VISIBILITY_FILE.to_owned(),
        source: EncodeError::from(e),
    })?;
    write_atomic(platform, &dir, VISIBILITY_FILE, &bytes)
}

/// What this device announces under `visibility` at `now`: tags (fresh
/// nonce) for the [`MAX_KNOWN_TAGS`] known devices most recently moved
/// with, plus its sanitised name and OS when shown to everyone. `None`
/// for [`Visibility::NoOne`], or when it would announce nothing (known
/// devices only, but none known).
///
/// # Errors
///
/// The known-device store is unreadable, or the OS random generator
/// failed.
pub fn advert(
    platform: &dyn Platform,
    visibility: Visibility,
    now: u64,
) -> Result<Option<PresenceAdvert>> {
    let everyone = match visibility.at(now) {
        Visibility::NoOne => return Ok(None),
        Visibility::Known => false,
        Visibility::Everyone { .. } => true,
    };
    let known = known_by_recent_use(platform, MAX_KNOWN_TAGS)?;
    let known = if known.is_empty() {
        None
    } else {
        let me = KnownDevices::open(platform)?.device_key(platform)?.public();
        KnownTags::fresh(known.iter().map(|(_, s)| s), &me)?
    };
    if !everyone {
        return Ok(known.map(|known| PresenceAdvert {
            known: Some(known),
            ..PresenceAdvert::default()
        }));
    }
    Ok(Some(PresenceAdvert {
        known,
        name: Some(announced_name(platform)),
        os: Some(platform.os()),
    }))
}

/// The name this computer announces to everyone: its own, sanitised, or
/// [`UNNAMED`] where the OS gives none (browsers drop an advert with
/// neither a name nor tags).
pub(crate) fn announced_name(platform: &dyn Platform) -> String {
    crate::about::device_name(platform)
        .as_deref()
        .and_then(clean_device_name)
        .unwrap_or_else(|| UNNAMED.to_owned())
}

/// What a computer without a device name is announced as.
const UNNAMED: &str = "Carrova device";

/// The background presence advert, for the app's lifetime: announces what
/// the saved visibility ([`save_visibility`]) allows, re-announces every
/// [`REFRESH`], and drops back to known devices by itself when "everyone"
/// ends. Stops on [`Self::stop`] or drop.
///
/// The saved state is the only source of the choice, so a stale value can
/// never override it. It follows the visibility and the known devices
/// wherever they were changed (the CLI too): at once when the state dir is
/// reported changed ([`Platform::watch_dirs`]) or on [`Self::refresh`],
/// else within [`POLL`].
///
/// The "everyone" deadline is wall-clock time, waited for with a
/// monotonic timer and re-checked against the clock on every wake: if the
/// clock jumps forward, the name stays visible until the timer fires (at
/// most [`REFRESH`]).
#[derive(Debug)]
pub struct Presence {
    /// Holds a sender too: dropped before `wakes` so the thread
    /// sees the channel close.
    watch: Option<DirWatch>,
    wakes: Option<mpsc::Sender<Wake>>,
    thread: Option<JoinHandle<()>>,
}

/// Why a running presence wakes, besides its advert's own deadline.
enum Wake {
    /// The saved state may have changed.
    Saved,
    /// The watch of the state dir ended: check every [`POLL`] from now on.
    Unwatched,
}

/// Held by a watch's callback: says [`Wake::Unwatched`] when the watch
/// drops it (it ended).
struct Watching(mpsc::Sender<Wake>);

impl Drop for Watching {
    fn drop(&mut self) {
        let _ = self.0.send(Wake::Unwatched);
    }
}

impl Presence {
    /// Start advertising for `engine`'s platform under the saved
    /// visibility. Failures are logged and leave the device unadvertised
    /// until the saved state next changes.
    #[must_use]
    pub fn start(engine: &Engine) -> Self {
        let platform = Arc::clone(engine.platform());
        let (tx, rx) = mpsc::channel();
        let watch = watch_saved(platform.as_ref(), tx.clone());
        let poll = watch.is_none().then_some(POLL);
        let thread = thread::Builder::new()
            .name("carrova-presence".into())
            .spawn(move || run(platform.as_ref(), &rx, poll))
            .inspect_err(|e| tracing::warn!(error = %e, "presence could not start"))
            .ok();
        Self {
            watch,
            wakes: Some(tx),
            thread,
        }
    }

    /// Read the saved state now and re-announce if it changed (after this
    /// process saved it, without waiting for the platform to report it).
    pub fn refresh(&self) {
        if let Some(tx) = &self.wakes {
            // The thread only ends when this handle drops the sender.
            let _ = tx.send(Wake::Saved);
        }
    }

    /// Stop advertising and wait for the advert to be withdrawn (as
    /// dropping it does).
    pub fn stop(self) {
        drop(self);
    }
}

impl Drop for Presence {
    fn drop(&mut self) {
        // Its end reaches the thread as `Unwatched`: at most one more read
        // before the channel closes.
        self.watch.take();
        self.wakes.take();
        if let Some(thread) = self.thread.take()
            && thread.join().is_err()
        {
            tracing::warn!("presence thread panicked");
        }
    }
}

/// Report a possible change of the saved state on `wakes` whenever the
/// state dir or the known-device dir changes. `None` (logged) where the
/// platform cannot watch them: then the caller polls.
fn watch_saved(platform: &dyn Platform, wakes: mpsc::Sender<Wake>) -> Option<DirWatch> {
    let watch = || -> Result<DirWatch> {
        let dir = platform.state_dir()?;
        // Watched before anything is saved: the dir must exist to be.
        ensure_dir(platform, &dir, 0o700)?;
        let dirs = [dir.join(DEVICES_DIR), dir];
        let watching = Watching(wakes);
        let changed = Box::new(move || {
            let _ = watching.0.send(Wake::Saved);
        });
        Ok(platform.watch_dirs(&dirs, changed)?)
    };
    watch()
        .inspect_err(
            |e| tracing::debug!(error = %e, "presence: saved state checked every {POLL:?}"),
        )
        .ok()
}

/// Advertise under the saved visibility, again at each [`next_wake`], and
/// at once when the saved visibility or known devices change (read on a
/// [`Wake`], or every `poll`), until every sender of `wakes` is dropped.
/// What is announced always comes from the last read that succeeded.
fn run(platform: &dyn Platform, wakes: &mpsc::Receiver<Wake>, mut poll: Option<Duration>) {
    let mut seen = Saved::read(platform);
    let mut visibility = seen
        .as_ref()
        .map_or_else(Visibility::default, |s| s.visibility);
    loop {
        let now = now_unix();
        // A new beacon each round (the last one is gone): new instance
        // name, fresh nonce. It announces until the end of the round.
        let _beacon = start(platform, visibility, now);
        let refresh_at = next_wake(visibility, now).map(|wait| Instant::now() + wait);
        loop {
            let wake = match wait(refresh_at, poll, Instant::now()) {
                Some(wait) => wakes.recv_timeout(wait),
                None => wakes.recv().map_err(|_| RecvTimeoutError::Disconnected),
            };
            match wake {
                Err(RecvTimeoutError::Disconnected) => return,
                // Also sent by a watch that failed to start (already
                // polling): nothing ended, nothing to read.
                Ok(Wake::Unwatched) if poll.is_some() => continue,
                Ok(Wake::Unwatched) => {
                    tracing::debug!("presence: watch ended, saved state checked every {POLL:?}");
                    poll = Some(POLL);
                }
                Ok(Wake::Saved) | Err(RecvTimeoutError::Timeout) => {}
            }
            let saved = Saved::read(platform);
            if saved != seen {
                if let Some(saved) = &saved {
                    visibility = saved.visibility;
                }
                seen = saved;
                break;
            }
            if refresh_at.is_some_and(|at| Instant::now() >= at) {
                break;
            }
        }
    }
}

/// What a running presence watches in the state dir: what its advert is
/// made from, so any change to it (a re-pairing's new secret too) is seen.
#[derive(PartialEq, Eq)]
struct Saved {
    /// The saved visibility in force.
    visibility: Visibility,
    /// The known devices tagged, each with a digest of its secret (the
    /// secret itself is not kept).
    known: Vec<(DeviceRecord, blake3::Hash)>,
}

impl Saved {
    /// Read now; `None` (logged) if either is unreadable.
    fn read(platform: &dyn Platform) -> Option<Self> {
        let read = || -> Result<Self> {
            Ok(Self {
                visibility: load_visibility(platform, now_unix())?,
                known: known_by_recent_use(platform, MAX_KNOWN_TAGS)?
                    .into_iter()
                    .map(|(record, secret)| (record, blake3::hash(secret.expose_bytes())))
                    .collect(),
            })
        };
        read()
            .inspect_err(|e| tracing::debug!(error = %e, "presence: saved state unreadable"))
            .ok()
    }
}

/// How long to wait before announcing again: [`REFRESH`], or less to meet
/// an "everyone" deadline; `None` (only on a change) when nothing is
/// announced.
fn next_wake(visibility: Visibility, now: u64) -> Option<Duration> {
    match visibility.at(now) {
        Visibility::NoOne => None,
        Visibility::Known => Some(REFRESH),
        Visibility::Everyone { until } => Some(REFRESH.min(Duration::from_secs(until - now))),
    }
}

/// How long to wait for a [`Wake`] at `now`: until `refresh_at`, or one
/// `poll` if sooner; `None` (no timer at all) when neither is set.
fn wait(refresh_at: Option<Instant>, poll: Option<Duration>, now: Instant) -> Option<Duration> {
    let due = refresh_at.map(|at| at.saturating_duration_since(now));
    due.into_iter().chain(poll).min()
}

/// A beacon announcing what `visibility` allows at `now` (none if
/// nothing, or on failure).
fn start(platform: &dyn Platform, visibility: Visibility, now: u64) -> Option<PresenceBeacon> {
    let advert = advert(platform, visibility, now)
        .inspect_err(|e| tracing::warn!(error = %e, "presence advert unavailable"))
        .ok()??;
    PresenceBeacon::start(platform, advert)
        .inspect_err(|e| tracing::warn!(error = %e, "presence could not start"))
        .ok()
}

#[cfg(test)]
#[path = "../tests/presence/unit.rs"]
mod tests;
