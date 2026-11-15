use super::*;
use std::io;

fn mimes(list: &[&str]) -> HashSet<String> {
    list.iter().map(|m| (*m).to_owned()).collect()
}

fn hint(m: &HashSet<String>, content: &[u8]) -> bool {
    is_sensitive(m, || Ok::<_, io::Error>(content.to_vec()))
}

#[test]
fn password_hint_marks_sensitive() {
    let m = mimes(&["text/plain", "x-kde-passwordManagerHint"]);
    assert!(hint(&m, b"secret"));
}

#[test]
fn hint_with_other_value_is_not_sensitive() {
    let m = mimes(&["text/plain", "x-kde-passwordManagerHint"]);
    assert!(!hint(&m, b"public"));
}

#[test]
fn unreadable_hint_fails_closed() {
    let m = mimes(&["text/plain", "x-kde-passwordManagerHint"]);
    assert!(is_sensitive(&m, || Err::<Vec<u8>, _>(io::Error::other(
        "gone"
    ))));
}

#[test]
fn no_hint_is_not_sensitive_and_never_reads() {
    let m = mimes(&["text/plain"]);
    assert!(!is_sensitive(&m, || -> Result<Vec<u8>, io::Error> {
        unreachable!("no hint to read")
    }));
}

#[test]
fn x11_is_the_fallback_when_data_control_is_missing() {
    assert!(matches!(select(true, || true), Clipboard::Wayland));
    assert!(matches!(select(true, || false), Clipboard::X11));
}

#[test]
fn no_wayland_session_never_probes() {
    assert!(matches!(
        select(false, || unreachable!("no compositor to probe")),
        Clipboard::X11
    ));
}
