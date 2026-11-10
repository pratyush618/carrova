use carrova_core::{CodeOffer, SendPrompt, Ticket};

use super::SendUi;
use crate::output::OutputFormat;

type Ask = fn(&str) -> anyhow::Result<bool>;

fn ui(format: OutputFormat, ask: Option<Ask>) -> SendUi<Vec<u8>, Vec<u8>, Ask> {
    SendUi::new(Vec::new(), Vec::new(), format, ask)
}

fn text(b: &[u8]) -> String {
    String::from_utf8(b.to_vec()).unwrap()
}

#[test]
fn wrong_code_warns_loudly_and_asks_before_a_new_code() {
    let mut yes = ui(OutputFormat::Human, Some(|q| Ok(q == "Show a new code?")));
    assert!(yes.confirm_new_code(1, 2));
    let err = text(&yes.err);
    assert!(
        err.contains("Someone entered a wrong code. If that wasn't you, stop now."),
        "{err}"
    );
    let mut no = ui(OutputFormat::Human, Some(|_| Ok(false)));
    assert!(!no.confirm_new_code(2, 1));
    let mut broken = ui(
        OutputFormat::Human,
        Some(|_| Err(anyhow::anyhow!("stdin closed"))),
    );
    assert!(!broken.confirm_new_code(1, 2), "unreadable answer is no");
}

#[test]
fn nobody_to_ask_means_no_new_code() {
    let mut headless = ui(OutputFormat::Human, None);
    assert!(!headless.confirm_new_code(1, 2));
    let err = text(&headless.err);
    assert!(
        err.contains("stop now") && err.contains("Not offering a new code"),
        "{err}"
    );

    let mut json = ui(OutputFormat::Json, None);
    assert!(!json.confirm_new_code(1, 2));
    assert!(json.err.is_empty() && json.out.is_empty());
}

#[test]
fn codes_go_to_the_terminal_or_the_json_record_only() {
    let ticket: Ticket = "7-acid-lemon".parse().unwrap();
    let mut human = ui(OutputFormat::Human, None);
    human.show_code(&ticket, CodeOffer::Renewed);
    assert!(human.out.is_empty());
    assert!(text(&human.err).contains("7-acid-lemon"));
    assert!(text(&human.err).contains("New code"));

    let mut json = ui(OutputFormat::Json, None);
    json.show_code(&ticket, CodeOffer::First);
    assert!(json.err.is_empty());
    let record: serde_json::Value = serde_json::from_slice(&json.out).unwrap();
    assert_eq!(record["pairing_code"], "7-acid-lemon");
    assert_eq!(record["renewed"], false);
}
