use carrova_platform::phone::{Hidden, Note};
use carrova_view::phone::{NotesView, notes_view, withheld_while_locked};

fn note(key: &str, icon: Option<Vec<u8>>, hidden: Option<Hidden>) -> Note {
    Note {
        key: key.to_owned(),
        app_id: "org.chat".to_owned(),
        app_name: "Chat".to_owned(),
        title: "Sam".to_owned(),
        text: "See you".to_owned(),
        when_ms: 1,
        can_reply: true,
        icon,
        hidden,
    }
}

#[test]
fn locked_notes_show_count_only() {
    let notes = [note("a", None, None), note("b", None, None)];
    assert_eq!(notes_view(&notes, true), NotesView::Locked { count: 2 });
}

#[test]
fn unlocked_notes_list_with_icon_as_png_data_url() {
    let notes = [note("a", Some(vec![0x89, b'P', b'N', b'G']), None)];
    let NotesView::List { notes: shown } = notes_view(&notes, false) else {
        panic!("not a list");
    };
    assert_eq!(shown.len(), 1);
    assert_eq!(shown[0].key, "a");
    assert_eq!(shown[0].title, "Sam");
    assert!(shown[0].can_reply);
    assert_eq!(
        shown[0].icon.as_deref(),
        Some("data:image/png;base64,iVBORw==")
    );
}

#[test]
fn note_without_icon_has_none() {
    let NotesView::List { notes } = notes_view(&[note("a", None, None)], false) else {
        panic!("not a list");
    };
    assert_eq!(notes[0].icon, None);
}

#[test]
fn withheld_while_locked_follows_the_engine_marks() {
    assert!(!withheld_while_locked(&[]));
    assert!(!withheld_while_locked(&[note(
        "a",
        None,
        Some(Hidden::Code)
    )]));
    assert!(withheld_while_locked(&[
        note("a", None, None),
        note("b", None, Some(Hidden::Locked)),
    ]));
}

#[test]
fn note_view_debug_hides_content() {
    let NotesView::List { notes } = notes_view(&[note("a", None, None)], false) else {
        panic!("not a list");
    };
    let printed = format!("{notes:?}");
    assert!(!printed.contains("Sam") && !printed.contains("See you") && !printed.contains("Chat"));
}
