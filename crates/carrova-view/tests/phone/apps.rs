use carrova_core::ecosystem::NotifyApps;
use carrova_platform::fake::FakePlatform;
use carrova_platform::phone::Note;
use carrova_view::phone::{NotifyAppRow, notify_apps_view};

const AUTH: &str = "com.authy.authy";

fn note(app_id: &str, app_name: &str) -> Note {
    Note {
        key: format!("k-{app_id}"),
        app_id: app_id.to_owned(),
        app_name: app_name.to_owned(),
        title: String::new(),
        text: String::new(),
        when_ms: 1,
        can_reply: false,
        icon: None,
        hidden: None,
    }
}

fn row(id: &str, name: &str, on: bool, private: bool) -> NotifyAppRow {
    NotifyAppRow {
        id: id.to_owned(),
        name: name.to_owned(),
        on,
        private,
    }
}

#[test]
fn private_app_off_by_default_in_list() {
    let tmp = tempfile::tempdir().unwrap();
    let p = FakePlatform::with_home(tmp.path()).with_state_dir(tmp.path().join("state"));
    let mut apps = NotifyApps::open(&p).unwrap();
    apps.set(&p, "org.gone", false).unwrap();
    let showing = [
        note("org.chat", "chat"),
        note(AUTH, "Authy"),
        note("org.chat", "chat"),
    ];

    let view = notify_apps_view(&apps, &showing);

    assert_eq!(
        view.apps,
        [
            row(AUTH, "Authy", false, true),
            row("org.chat", "chat", true, false),
            row("org.gone", "org.gone", false, false),
        ]
    );
    assert!(view.hide_codes && view.show_text);
}
