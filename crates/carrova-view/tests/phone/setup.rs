use carrova_platform::phone::{CallAudioStatus, PermState, PhonePerm};
use carrova_view::phone::{CallSetupStep, PermStep, call_audio_view, perm_view};

#[test]
fn setup_step_per_status() {
    let table = [
        (CallAudioStatus::NoAdapter, CallSetupStep::NoBt),
        (CallAudioStatus::BluetoothOff, CallSetupStep::Off),
        (CallAudioStatus::NoHandsFree, CallSetupStep::NoBt),
        (CallAudioStatus::NeedsPackage, CallSetupStep::Install),
        (CallAudioStatus::NeedsPairing, CallSetupStep::Pair),
        (CallAudioStatus::Ready, CallSetupStep::Done),
        (CallAudioStatus::Unsupported, CallSetupStep::NoBt),
    ];
    for (status, step) in table {
        assert_eq!(call_audio_view(status), step, "{status:?}");
    }
}

const PERMS: [PhonePerm; 5] = [
    PhonePerm::NotificationAccess,
    PhonePerm::Sms,
    PhonePerm::Calls,
    PhonePerm::Contacts,
    PhonePerm::BluetoothConnect,
];

#[test]
fn restricted_perm_shows_restricted_step() {
    for perm in PERMS {
        assert_eq!(perm_view(perm, PermState::Restricted), PermStep::Restricted);
    }
}

#[test]
fn granted_perm_is_ok() {
    for perm in PERMS {
        assert_eq!(perm_view(perm, PermState::Granted), PermStep::Ok);
    }
}

#[test]
fn denied_access_opens_settings_and_runtime_perms_ask() {
    let table = [
        (PhonePerm::NotificationAccess, PermStep::OpenSettings),
        (PhonePerm::Sms, PermStep::Ask),
        (PhonePerm::Calls, PermStep::Ask),
        (PhonePerm::Contacts, PermStep::Ask),
        (PhonePerm::BluetoothConnect, PermStep::Ask),
    ];
    for (perm, step) in table {
        assert_eq!(perm_view(perm, PermState::Denied), step, "{perm:?}");
    }
}
