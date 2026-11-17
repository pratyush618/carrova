use carrova_platform::fake::FakePlatform;
use carrova_platform::phone::{Hidden, Note};

use super::{NotifyApps, outgoing};

fn note(title: &str, text: &str) -> Note {
    Note {
        key: "k".into(),
        app_id: "org.example.chat".into(),
        app_name: "Chat".into(),
        title: title.into(),
        text: text.into(),
        when_ms: 0,
        can_reply: false,
        icon: None,
        hidden: None,
    }
}

fn apps() -> (tempfile::TempDir, FakePlatform, NotifyApps) {
    let tmp = tempfile::tempdir().unwrap();
    let p = FakePlatform::with_home(tmp.path()).with_state_dir(tmp.path().join("state"));
    let apps = NotifyApps::open(&p).unwrap();
    (tmp, p, apps)
}

#[test]
fn app_off_sends_nothing() {
    let (_t, p, mut apps) = apps();
    apps.set(&p, "org.example.chat", false).unwrap();
    assert!(outgoing(note("Hi", "there"), &apps).is_none());
}

#[test]
fn plain_note_passes_unchanged() {
    let (_t, _p, apps) = apps();
    assert_eq!(
        outgoing(note("Hi", "there"), &apps).unwrap(),
        note("Hi", "there")
    );
}

#[test]
fn show_text_off_strips_text() {
    let (_t, p, mut apps) = apps();
    apps.set_filters(&p, true, false).unwrap();
    let out = outgoing(note("Ann", "lunch?"), &apps).unwrap();
    assert_eq!((out.title.as_str(), out.text.as_str()), ("Ann", ""));
    assert_eq!(out.hidden, Some(Hidden::TextOff));
}

#[test]
fn otp_clears_title() {
    let (_t, _p, apps) = apps();
    let out = outgoing(note("Verification code 123456", ""), &apps).unwrap();
    assert_eq!((out.title.as_str(), out.text.as_str()), ("", ""));
    assert_eq!(out.hidden, Some(Hidden::Code));
    let out = outgoing(note("Bank", "OTP 482913"), &apps).unwrap();
    assert!(out.text.is_empty() && out.title.is_empty());
}

#[test]
fn otp_split_between_title_and_text_is_hidden() {
    let (_t, _p, apps) = apps();
    let out = outgoing(note("Verification code", "123456"), &apps).unwrap();
    assert_eq!((out.title.as_str(), out.text.as_str()), ("", ""));
    assert_eq!(out.hidden, Some(Hidden::Code));
}

#[test]
fn hide_codes_off_keeps_code() {
    let (_t, p, mut apps) = apps();
    apps.set_filters(&p, false, true).unwrap();
    assert_eq!(
        outgoing(note("Bank", "OTP 482913"), &apps).unwrap().text,
        "OTP 482913"
    );
}
