//! This device's presence on the LAN outside sessions, as far as the
//! user's [`Visibility`] allows (ADR 0001 §5): known devices recognise it
//! by tags only they can check; everyone else sees its name and OS only
//! while the user shows it to everyone.

use std::sync::Arc;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use carrova_platform::Platform;
use carrova_transport::{KnownTags, MAX_KNOWN_TAGS, PresenceAdvert, PresenceBeacon};

use crate::devices::KnownDevices;
use crate::engine::Engine;
use crate::error::Result;
use crate::lan::{clean_device_name, known_by_recent_use};
use crate::time::now_unix;

/// How often a running [`Presence`] re-announces itself under a new mDNS
/// instance with fresh tags, so its adverts cannot be linked over a long
/// time by `kt` or instance name (each restart costs one mDNS announce).
pub const REFRESH: Duration = Duration::from_secs(10 * 60);

/// Who may find this device nearby.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
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
        name: crate::about::device_name(platform)
            .as_deref()
            .and_then(clean_device_name),
        os: Some(platform.os()),
    }))
}

/// The background presence advert, for the app's lifetime: follows the
/// visibility it is [`set`](Self::set) to, re-announces every [`REFRESH`],
/// and drops back to known devices by itself when "everyone" ends. Stops
/// on [`Self::stop`] or drop.
///
/// It reads the known devices only when it announces: after remembering
/// or forgetting a device, call [`set`](Self::set) (with the current
/// visibility) so the tags follow at once.
///
/// The "everyone" deadline is wall-clock time, waited for with a
/// monotonic timer and re-checked against the clock on every wake: if the
/// clock jumps forward, the name stays visible until the timer fires (at
/// most [`REFRESH`]).
#[derive(Debug)]
pub struct Presence {
    visibility: Option<mpsc::Sender<Visibility>>,
    thread: Option<JoinHandle<()>>,
}

impl Presence {
    /// Start advertising for `engine`'s platform under `visibility`.
    /// Failures are logged and leave the device unadvertised until the
    /// next [`set`](Self::set).
    #[must_use]
    pub fn start(engine: &Engine, visibility: Visibility) -> Self {
        let platform = Arc::clone(engine.platform());
        let (tx, rx) = mpsc::channel();
        let thread = thread::Builder::new()
            .name("carrova-presence".into())
            .spawn(move || run(platform.as_ref(), visibility, &rx))
            .inspect_err(|e| tracing::warn!(error = %e, "presence could not start"))
            .ok();
        Self {
            visibility: Some(tx),
            thread,
        }
    }

    /// Advertise under `visibility` from now on (fresh instance and tags,
    /// the known devices read again).
    pub fn set(&self, visibility: Visibility) {
        if let Some(tx) = &self.visibility {
            // The thread only ends when this handle drops the sender.
            let _ = tx.send(visibility);
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
        self.visibility.take();
        if let Some(thread) = self.thread.take()
            && thread.join().is_err()
        {
            tracing::warn!("presence thread panicked");
        }
    }
}

/// Advertise under `visibility`, again at each [`next_wake`], until the
/// sender of `changes` is dropped.
fn run(platform: &dyn Platform, mut visibility: Visibility, changes: &mpsc::Receiver<Visibility>) {
    loop {
        let now = now_unix();
        // A new beacon each round (the last one is gone): new instance
        // name, fresh nonce. It announces until the end of the round.
        let _beacon = start(platform, visibility, now);
        let change = match next_wake(visibility, now) {
            Some(wait) => changes.recv_timeout(wait),
            None => changes.recv().map_err(|_| RecvTimeoutError::Disconnected),
        };
        match change {
            Ok(next) => visibility = next,
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}

/// How long to wait before announcing again: [`REFRESH`], or less to meet
/// an "everyone" deadline; `None` (until told) when nothing is announced.
fn next_wake(visibility: Visibility, now: u64) -> Option<Duration> {
    match visibility.at(now) {
        Visibility::NoOne => None,
        Visibility::Known => Some(REFRESH),
        Visibility::Everyone { until } => Some(REFRESH.min(Duration::from_secs(until - now))),
    }
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
