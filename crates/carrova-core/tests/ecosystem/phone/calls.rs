//! The laptop's calls controller: a scripted phone linked to a laptop,
//! and the one call view the laptop's windows, home bar and tray render.

use carrova_core::Event;
use carrova_core::ecosystem::{CallView, PeerState, PhoneSettings, RingHere, Supervisor};
use carrova_link::{CallCommand, CallReply, Feature, FeatureState, PolicyDoc};
use carrova_platform::ScreenLock;
use carrova_platform::phone::{AudioSide, CallState, PhoneEvent};

use crate::supervisor::device::{Device, pair, start_both, wait_until};

const NUMBER: &str = "5550100";

/// A phone and a laptop with calls on, linked, the phone watched:
/// `(phone, its supervisor, laptop, its supervisor)`.
fn linked() -> (Device, Supervisor, Device, Supervisor) {
    let mut doc = PolicyDoc::computer_defaults();
    doc.features.insert(Feature::Calls, FeatureState::On);
    let (laptop, phone, _) = pair(&doc);
    let (sl, sp) = start_both(&laptop, &phone);
    wait_until("both hold the other's policy", || {
        let has = |s: &Supervisor| {
            s.status()
                .iter()
                .any(|p| p.state == PeerState::Linked && p.remote.is_some())
        };
        has(&sl) && has(&sp)
    });
    wait_until("the phone's events watched", || phone.platform.watching());
    (phone, sp, laptop, sl)
}

fn ringing() -> CallState {
    CallState::Ringing {
        number: NUMBER.into(),
        name: Some("Ana".into()),
    }
}

fn active() -> CallState {
    CallState::Active {
        since_ms: 1_000,
        muted: false,
        on_laptop_audio: false,
    }
}

/// The phone's call becomes `state`, as the phone reports it.
fn phone_call(phone: &Device, state: CallState) {
    phone.platform.phone().call = state.clone();
    phone.platform.script_event(PhoneEvent::Call(state));
}

/// Wait for the call view `view`.
fn wait_view(laptop: &Device, view: &CallView) {
    laptop.wait("the call view", |e| *e == Event::CallView(view.clone()));
}

fn ringing_view(phone: &Device, ring_here: bool) -> CallView {
    CallView::Ringing {
        device: phone.id,
        number: NUMBER.into(),
        name: Some("Ana".into()),
        ring_here,
    }
}

fn active_view(phone: &Device, audio: AudioSide) -> CallView {
    CallView::Active {
        device: phone.id,
        since_ms: 1_000,
        muted: false,
        audio,
    }
}

/// The call view shown last.
fn last_view(laptop: &Device) -> Option<CallView> {
    laptop.events().into_iter().rev().find_map(|e| match e {
        Event::CallView(v) => Some(v),
        _ => None,
    })
}

#[test]
fn ringing_then_answer_then_end() {
    let (phone, _sp, laptop, sl) = linked();
    phone_call(&phone, ringing());
    wait_view(&laptop, &ringing_view(&phone, true));

    phone.platform.phone().answered = Some(active());
    assert_eq!(
        sl.call(&phone.id, &CallCommand::Answer).unwrap(),
        CallReply::Done
    );
    phone_call(&phone, active());
    wait_view(&laptop, &active_view(&phone, AudioSide::Phone));

    assert_eq!(
        sl.call(&phone.id, &CallCommand::End).unwrap(),
        CallReply::Done
    );
    phone_call(&phone, CallState::Ended { missed: false });
    wait_until("the call view cleared", || {
        last_view(&laptop) == Some(CallView::None)
    });
    let actions = phone.platform.actions();
    assert!(actions.iter().any(|a| a == "call_answer"), "{actions:?}");
    assert!(actions.iter().any(|a| a == "call_end"), "{actions:?}");
}

#[test]
fn decline_from_laptop() {
    let (phone, _sp, laptop, sl) = linked();
    phone_call(&phone, ringing());
    wait_view(&laptop, &ringing_view(&phone, true));

    assert_eq!(
        sl.call(&phone.id, &CallCommand::Decline).unwrap(),
        CallReply::Done
    );
    phone_call(&phone, CallState::Ended { missed: false });

    wait_until("the call view cleared", || {
        last_view(&laptop) == Some(CallView::None)
    });
    assert!(phone.platform.actions().iter().any(|a| a == "call_decline"));
    assert_eq!(
        laptop.count(|e| matches!(e, Event::CallView(CallView::Missed { .. }))),
        0
    );
}

#[test]
fn missed_after_ringing_ends_unanswered() {
    let (phone, _sp, laptop, _sl) = linked();
    phone_call(&phone, ringing());
    wait_view(&laptop, &ringing_view(&phone, true));

    phone_call(&phone, CallState::Ended { missed: true });

    wait_view(
        &laptop,
        &CallView::Missed {
            device: phone.id,
            number: NUMBER.into(),
            name: Some("Ana".into()),
        },
    );
}

#[test]
fn ringing_clears_when_link_drops() {
    let (phone, sp, laptop, _sl) = linked();
    phone_call(&phone, ringing());
    wait_view(&laptop, &ringing_view(&phone, true));

    sp.stop();

    wait_view(&laptop, &CallView::Left { device: phone.id });
}

#[test]
fn never_setting_does_not_ring_here() {
    let (phone, _sp, laptop, sl) = linked();
    let never = PhoneSettings {
        ring_here: RingHere::Never,
        ..PhoneSettings::default()
    };
    sl.set_phone_settings(never).unwrap();

    phone_call(&phone, ringing());

    wait_view(&laptop, &ringing_view(&phone, false));
}

#[test]
fn while_active_rings_unless_locked() {
    let (phone, _sp, laptop, sl) = linked();
    assert_eq!(sl.phone_settings().ring_here, RingHere::WhileActive);
    laptop.platform.inner.set_screen_lock(ScreenLock::Locked);
    phone_call(&phone, ringing());
    wait_view(&laptop, &ringing_view(&phone, false));

    // The OS can't say: it rings.
    laptop.platform.inner.set_screen_lock(ScreenLock::Unknown);
    phone_call(&phone, CallState::Idle);
    phone_call(&phone, ringing());
    wait_view(&laptop, &ringing_view(&phone, true));
}

#[test]
fn tones_go_to_local_audio() {
    let (phone, _sp, laptop, sl) = linked();
    phone_call(&phone, active());
    wait_view(&laptop, &active_view(&phone, AudioSide::Phone));

    let tones = CallCommand::Tones("12#".into());
    assert_eq!(sl.call(&phone.id, &tones).unwrap(), CallReply::Done);

    assert!(laptop.platform.actions().iter().any(|a| a == "call_tones"));
    assert!(!phone.platform.actions().iter().any(|a| a == "call_tones"));
}

#[test]
fn audio_side_follows_the_route() {
    let (phone, _sp, laptop, sl) = linked();
    phone_call(&phone, active());
    wait_view(&laptop, &active_view(&phone, AudioSide::Phone));

    // Moved here from the laptop: shown at once.
    laptop.platform.phone().side = AudioSide::Laptop;
    let to_laptop = CallCommand::AudioTo(AudioSide::Laptop);
    assert_eq!(sl.call(&phone.id, &to_laptop).unwrap(), CallReply::Done);
    assert!(
        laptop
            .platform
            .actions()
            .iter()
            .any(|a| a == "call_audio_to Laptop")
    );
    wait_view(&laptop, &active_view(&phone, AudioSide::Laptop));

    // The link drops (the phone took the audio back): the poll shows it.
    laptop.platform.phone().side = AudioSide::Phone;
    let before = laptop.count(|e| *e == Event::CallView(active_view(&phone, AudioSide::Phone)));
    wait_until("the audio back on the phone", || {
        laptop.count(|e| *e == Event::CallView(active_view(&phone, AudioSide::Phone))) > before
    });
}

#[test]
fn call_view_debug_hides_the_caller() {
    let phone = Device::new("Phone");
    let views = [
        ringing_view(&phone, true),
        CallView::Missed {
            device: phone.id,
            number: NUMBER.into(),
            name: Some("Ana".into()),
        },
    ];
    for view in views {
        let shown = format!("{view:?}");
        assert!(!shown.contains(NUMBER) && !shown.contains("Ana"), "{shown}");
    }
}
