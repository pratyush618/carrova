//! The user's clipboard settings: saved, and honoured live by the
//! supervisor (auto-send, the arrival note, the history file).

use carrova_core::Event;
use carrova_core::ecosystem::ClipSettings;
use carrova_link::{ClipItem, PolicyDoc};
use carrova_platform::Platform;

use super::device::{Device, fast, pair, wait_until};
use super::{clipboard, linked, text};

fn history_file(device: &Device) -> std::path::PathBuf {
    device
        .platform
        .state_dir()
        .unwrap()
        .join("clip-history.cbor")
}

#[test]
fn clip_settings_default_off_except_share_and_persist() {
    let (low, _high, _) = pair(&PolicyDoc::phone_defaults());
    let sl = low.start(fast());
    assert_eq!(
        sl.clip_settings(),
        ClipSettings {
            share: true,
            show_note: false,
            history_file: false,
        }
    );
    assert_eq!(sl.clip_settings(), ClipSettings::default());

    let changed = ClipSettings {
        share: false,
        show_note: true,
        history_file: true,
    };
    sl.set_clip_settings(changed).unwrap();
    sl.stop();

    assert_eq!(low.start(fast()).clip_settings(), changed);
}

#[test]
fn share_off_stops_auto_send_but_not_a_manual_send() {
    let (low, high, sl, _sh) = linked(&PolicyDoc::phone_defaults());
    sl.set_clip_settings(ClipSettings {
        share: false,
        ..ClipSettings::default()
    })
    .unwrap();

    low.platform.clipboard_write(&text("kept here")).unwrap();
    low.source.wait_rounds(10);
    assert_eq!(high.count(|e| matches!(e, Event::ClipReceived { .. })), 0);

    sl.send_clipboard(&high.id).unwrap();
    high.wait("clip", |e| matches!(e, Event::ClipReceived { .. }));
    assert_eq!(clipboard(&high).as_deref(), Some(&b"kept here"[..]));
}

#[test]
fn clip_received_carries_the_show_note_setting() {
    let (low, high, _sl, sh) = linked(&PolicyDoc::phone_defaults());

    low.platform.clipboard_write(&text("quiet")).unwrap();
    high.wait("quiet clip", |e| {
        *e == Event::ClipReceived {
            device: low.id,
            note: false,
        }
    });

    sh.set_clip_settings(ClipSettings {
        show_note: true,
        ..ClipSettings::default()
    })
    .unwrap();
    low.platform.clipboard_write(&text("noted")).unwrap();
    high.wait("noted clip", |e| {
        *e == Event::ClipReceived {
            device: low.id,
            note: true,
        }
    });
}

#[test]
fn history_file_is_written_only_while_on() {
    let (low, high, _sl, sh) = linked(&PolicyDoc::phone_defaults());

    low.platform.clipboard_write(&text("memory only")).unwrap();
    high.wait("first clip", |e| matches!(e, Event::ClipReceived { .. }));
    assert!(!history_file(&high).exists(), "off: no file");

    let on = ClipSettings {
        history_file: true,
        ..ClipSettings::default()
    };
    sh.set_clip_settings(on).unwrap();
    assert!(history_file(&high).exists(), "on: the history is written");
    low.platform.clipboard_write(&text("kept")).unwrap();
    wait_until("second clip", || sh.clip_history().len() == 2);
    sh.stop();

    let sh = high.start(fast());
    let kept: Vec<_> = sh.clip_history().into_iter().map(|c| c.item).collect();
    assert_eq!(
        kept,
        [
            ClipItem::Text("kept".to_owned()),
            ClipItem::Text("memory only".to_owned())
        ]
    );

    sh.set_clip_settings(ClipSettings::default()).unwrap();
    assert!(
        !history_file(&high).exists(),
        "off again: the file is deleted"
    );
    assert_eq!(sh.clip_history().len(), 2, "the memory keeps it");
}

#[test]
fn clearing_the_history_clears_its_file() {
    let (low, high, _sl, sh) = linked(&PolicyDoc::phone_defaults());
    sh.set_clip_settings(ClipSettings {
        history_file: true,
        ..ClipSettings::default()
    })
    .unwrap();
    low.platform.clipboard_write(&text("gone soon")).unwrap();
    high.wait("clip", |e| matches!(e, Event::ClipReceived { .. }));

    sh.clear_clip_history();
    sh.stop();

    assert!(high.start(fast()).clip_history().is_empty());
}
