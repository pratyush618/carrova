use super::*;

fn report(messages: &[&str]) -> ErrorReport {
    ErrorReport {
        app_version: "0.4.0".into(),
        os: "Linux".into(),
        when: 1,
        kind: ReportKind::Panic,
        message_chain: messages.iter().map(ToString::to_string).collect(),
        backtrace_frames: vec!["carrova_core::engine::run".into()],
    }
}

#[test]
fn url_targets_the_new_issue_page_with_encoded_title_and_body() {
    let draft = issue_draft(&report(&["boom & bust?"]));

    assert_eq!(draft.title, "Crash: boom & bust?");
    assert!(draft.url.starts_with(&format!(
        "https://github.com/{ISSUE_REPOSITORY}/issues/new?title=Crash%3A%20boom%20%26%20bust%3F&body="
    )));
    assert!(draft.body.contains("- boom & bust?\n"));
    assert!(draft.body.contains("carrova_core::engine::run"));
    assert!(draft.url.ends_with(&percent_encode(&draft.body)));
}

#[test]
fn long_bodies_are_truncated_to_fit_the_url_budget() {
    let long = "é".repeat(5000);
    let draft = issue_draft(&report(&[&long]));

    assert!(percent_encode(&draft.body).len() <= MAX_ENCODED_BODY);
    assert!(draft.body.ends_with(TRUNCATION_NOTE));
}

#[test]
fn percent_encoding_keeps_only_unreserved_characters() {
    assert_eq!(percent_encode("a-Z_9.~ /\n%é"), "a-Z_9.~%20%2F%0A%25%C3%A9");
}
