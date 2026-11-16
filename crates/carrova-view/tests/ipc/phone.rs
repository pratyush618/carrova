use carrova_link::{ActionResult, AudioSide, CallCommand, CallReply, NotifyAction};
use carrova_view::ipc::{CallAction, CallOutcome, NoteAction, NoteOutcome};

fn action(json: &str) -> NoteAction {
    serde_json::from_str(json).unwrap()
}

fn call(json: &str) -> CallCommand {
    serde_json::from_str::<CallAction>(json).unwrap().into()
}

#[test]
fn note_actions_reach_the_phone_unchanged() {
    let reply = action(r#"{"kind":"reply","key":"k","text":"ok"}"#);
    assert_eq!(
        NotifyAction::from(reply),
        NotifyAction::Reply {
            key: "k".to_owned(),
            text: "ok".to_owned()
        }
    );
    assert_eq!(
        NotifyAction::from(action(r#"{"kind":"dismiss","key":"k"}"#)),
        NotifyAction::Dismiss {
            key: "k".to_owned()
        }
    );
    assert_eq!(
        NotifyAction::from(action(r#"{"kind":"open","key":"k"}"#)),
        NotifyAction::Open {
            key: "k".to_owned()
        }
    );
}

#[test]
fn call_actions_become_call_commands() {
    let table = [
        (r#"{"kind":"answer"}"#, CallCommand::Answer),
        (r#"{"kind":"decline"}"#, CallCommand::Decline),
        (r#"{"kind":"end"}"#, CallCommand::End),
        (r#"{"kind":"mute","on":true}"#, CallCommand::Mute(true)),
        (
            r#"{"kind":"dial","number":"+15550142"}"#,
            CallCommand::Dial {
                number: "+15550142".to_owned(),
            },
        ),
        (
            r#"{"kind":"tones","digits":"12#"}"#,
            CallCommand::Tones("12#".to_owned()),
        ),
        (
            r#"{"kind":"audio_to","side":"laptop"}"#,
            CallCommand::AudioTo(AudioSide::Laptop),
        ),
    ];
    for (json, command) in table {
        assert_eq!(call(json), command, "{json}");
    }
}

#[test]
fn outcomes_follow_the_phone_replies() {
    let notes = [
        (ActionResult::Done, NoteOutcome::Done),
        (ActionResult::Gone, NoteOutcome::Gone),
        (ActionResult::Failed, NoteOutcome::Failed),
    ];
    for (result, outcome) in notes {
        assert_eq!(NoteOutcome::from(result), outcome);
    }
    let calls = [
        (CallReply::Done, CallOutcome::Done),
        (CallReply::NotRinging, CallOutcome::NotRinging),
        (CallReply::NoCall, CallOutcome::NoCall),
        (CallReply::Failed, CallOutcome::Failed),
        // A page answers no action: never reported as done.
        (CallReply::HistoryPage(Vec::new()), CallOutcome::Failed),
    ];
    for (reply, outcome) in calls {
        assert_eq!(CallOutcome::from(reply), outcome);
    }
}

#[test]
fn note_action_debug_hides_key_and_text() {
    let printed = format!(
        "{:?}",
        action(r#"{"kind":"reply","key":"k1","text":"secret"}"#)
    );
    assert!(!printed.contains("secret") && !printed.contains("k1"));
}
