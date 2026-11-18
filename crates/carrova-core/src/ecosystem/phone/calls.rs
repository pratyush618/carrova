//! The laptop's calls controller: turns a linked phone's call state, the
//! "Ring on this computer" setting and this computer's audio route into
//! one [`CallView`], the single source of truth the ringing window, call
//! window, home bar and tray render.
//!
//! Never logs numbers, names or call details.

use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use carrova_crypto::identity::DeviceId;
use carrova_link::{Feature, Pending};
use carrova_platform::phone::{AudioSide, CallState};
use carrova_platform::{Platform, ScreenLock};
use serde::{Deserialize, Serialize};

use super::RingHere;
use super::mirror::{ack, with_mirror};
use crate::ecosystem::supervisor::Shared;
use crate::session::Event;

/// How often an active call's audio side is read again, so a dropped
/// Bluetooth audio link shows.
const AUDIO_POLL: Duration = Duration::from_secs(1);

/// What this computer shows of a linked phone's call
/// ([`Event::CallView`]). `Debug` hides the caller.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CallView {
    /// No call.
    None,
    /// An incoming call is ringing on the phone.
    Ringing {
        #[cfg_attr(feature = "ts", ts(type = "string"))]
        device: DeviceId,
        /// Caller's number.
        number: String,
        /// Caller's contact name, resolved on the phone.
        name: Option<String>,
        /// Ring here too ("Ring on this computer"), not only show it.
        /// Decided when the phone reports the ringing, not again as the
        /// screen locks or unlocks.
        ring_here: bool,
    },
    /// A call is in progress.
    Active {
        #[cfg_attr(feature = "ts", ts(type = "string"))]
        device: DeviceId,
        /// When it was answered, or dialled when `outgoing` (ms since the
        /// Unix epoch).
        since_ms: i64,
        /// The microphone is muted.
        muted: bool,
        /// Where its audio plays now.
        audio: AudioSide,
        /// Placed from the phone: when it was answered is unknown.
        #[serde(default)]
        outgoing: bool,
    },
    /// The ringing call ended unanswered. Needs the ringing held in
    /// memory: a link replaced between the ringing and its end shows no
    /// missed call.
    Missed {
        #[cfg_attr(feature = "ts", ts(type = "string"))]
        device: DeviceId,
        /// Caller's number.
        number: String,
        /// Caller's contact name, resolved on the phone.
        name: Option<String>,
    },
    /// The ringing or active call showing is gone from here (link closed,
    /// calls turned off or the device forgotten): its windows close.
    Left {
        #[cfg_attr(feature = "ts", ts(type = "string"))]
        device: DeviceId,
    },
}

impl fmt::Debug for CallView {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => f.write_str("None"),
            Self::Ringing { ring_here, .. } => f
                .debug_struct("Ringing")
                .field("ring_here", ring_here)
                .finish_non_exhaustive(),
            Self::Active {
                muted,
                audio,
                outgoing,
                ..
            } => f
                .debug_struct("Active")
                .field("muted", muted)
                .field("audio", audio)
                .field("outgoing", outgoing)
                .finish_non_exhaustive(),
            Self::Missed { .. } => f.debug_struct("Missed").finish_non_exhaustive(),
            Self::Left { .. } => f.debug_struct("Left").finish_non_exhaustive(),
        }
    }
}

/// A phone's call as one link's mirror holds it.
#[derive(Default)]
pub(crate) struct Call {
    /// The phone's call, once reported.
    pub(super) state: Option<CallState>,
    /// The audio side shown for the active call.
    audio: Option<AudioSide>,
    /// An audio poll runs for this link ([`watch_audio`]).
    polling: bool,
}

impl Call {
    /// Take the phone's `state`: the view it shows (`ring_here` and `audio`
    /// read for it beforehand).
    fn take(
        &mut self,
        device: DeviceId,
        state: CallState,
        ring_here: bool,
        audio: AudioSide,
    ) -> CallView {
        let before = self.state.replace(state.clone());
        match (state, before) {
            (CallState::Ringing { number, name }, _) => CallView::Ringing {
                device,
                number,
                name,
                ring_here,
            },
            (
                CallState::Active {
                    since_ms,
                    muted,
                    outgoing,
                    ..
                },
                _,
            ) => {
                self.audio = Some(audio);
                CallView::Active {
                    device,
                    since_ms,
                    muted,
                    audio,
                    outgoing,
                }
            }
            (CallState::Ended { missed: true }, Some(CallState::Ringing { number, name })) => {
                CallView::Missed {
                    device,
                    number,
                    name,
                }
            }
            (CallState::Idle | CallState::Ended { .. }, _) => CallView::None,
        }
    }

    /// A ringing or active call shows: dropping it shows
    /// [`CallView::Left`]. An idle or ended call holds nothing to close.
    pub(super) fn shown(&self) -> bool {
        matches!(
            self.state,
            Some(CallState::Ringing { .. } | CallState::Active { .. })
        )
    }

    fn active(&self) -> bool {
        matches!(self.state, Some(CallState::Active { .. }))
    }

    /// The active call's view with its audio at `audio`, if that moved it.
    fn audio_now(&mut self, device: DeviceId, audio: AudioSide) -> Option<CallView> {
        let Some(CallState::Active {
            since_ms,
            muted,
            outgoing,
            ..
        }) = self.state
        else {
            return None;
        };
        if self.audio.replace(audio) == Some(audio) {
            return None;
        }
        Some(CallView::Active {
            device,
            since_ms,
            muted,
            audio,
            outgoing,
        })
    }
}

/// Whether a ringing call rings here now. While active: unless the screen
/// is locked (no idle signal exists; when the OS can't say, it rings).
fn rings_here(shared: &Shared) -> bool {
    match shared.phone_settings().ring_here {
        RingHere::Always => true,
        RingHere::Never => false,
        RingHere::WhileActive => shared.platform().screen_lock() != ScreenLock::Locked,
    }
}

/// Where the call's audio plays; on the phone when this computer can't say.
fn audio_side(platform: &dyn Platform) -> AudioSide {
    platform.call_audio_side().unwrap_or(AudioSide::Phone)
}

/// Apply the call state from `device`'s link `generation` and show it,
/// then acknowledge it.
pub(crate) fn take_call(
    shared: &Arc<Shared>,
    device: DeviceId,
    generation: u64,
    state: CallState,
    pending: Pending,
) {
    // Read outside the supervisor's lock: the OS may be slow to answer.
    let ring_here = matches!(state, CallState::Ringing { .. }) && rings_here(shared);
    let audio = match state {
        CallState::Active { .. } => audio_side(&**shared.platform()),
        _ => AudioSide::Phone,
    };
    // Lock order: call views, then peers (in `with_mirror`).
    let views = shared.call_views();
    let poll = with_mirror(shared, device, generation, Feature::Calls, |m| {
        let view = m.call.take(device, state.clone(), ring_here, audio);
        // Under the lock every clear takes: never after its `Left`.
        shared.emit(Event::PhoneCall { device, state });
        shared.emit(Event::CallView(view));
        m.call.active() && !std::mem::replace(&mut m.call.polling, true)
    });
    drop(views);
    if poll == Some(true) {
        watch_audio(shared, device, generation);
    }
    ack(pending);
}

/// Read the audio side of `device`'s call again and show it if it moved;
/// whether the call is still active. A `poller` that sees it is not ends
/// its poll.
pub(crate) fn reread_audio(
    shared: &Shared,
    device: DeviceId,
    generation: u64,
    poller: bool,
) -> bool {
    let audio = audio_side(&**shared.platform());
    let _views = shared.call_views();
    let active = with_mirror(shared, device, generation, Feature::Calls, |m| {
        let active = m.call.active();
        if poller && !active {
            m.call.polling = false;
        }
        if let Some(view) = m.call.audio_now(device, audio) {
            // Under the lock every clear takes: never after its `Left`.
            shared.emit(Event::CallView(view));
        }
        active
    });
    active == Some(true)
}

/// Poll the audio side of `device`'s active call until it ends, its link
/// goes or the supervisor stops.
fn watch_audio(shared: &Arc<Shared>, device: DeviceId, generation: u64) {
    let polling = Arc::clone(shared);
    let started = shared.spawn("carrova-call-audio", move || {
        while polling.stop.sleep(AUDIO_POLL).is_ok()
            && reread_audio(&polling, device, generation, true)
        {}
    });
    if let Err(e) = started {
        tracing::debug!(error = %e, "call audio not watched");
        with_mirror(shared, device, generation, Feature::Calls, |m| {
            m.call.polling = false;
        });
    }
}
