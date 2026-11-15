use super::*;

fn mimes(list: &[&str]) -> HashSet<String> {
    list.iter().map(|m| (*m).to_owned()).collect()
}

#[test]
fn password_hint_marks_sensitive() {
    let m = mimes(&["text/plain", "x-kde-passwordManagerHint"]);
    assert!(is_sensitive(&m, || b"secret".to_vec()));
}

#[test]
fn hint_with_other_value_is_not_sensitive() {
    let m = mimes(&["text/plain", "x-kde-passwordManagerHint"]);
    assert!(!is_sensitive(&m, || b"public".to_vec()));
}

#[test]
fn no_hint_is_not_sensitive_and_never_reads() {
    let m = mimes(&["text/plain"]);
    assert!(!is_sensitive(&m, || unreachable!("no hint to read")));
}
