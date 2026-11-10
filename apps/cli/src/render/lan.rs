//! Text rendering of the LAN views in [`carrova_view::lan`]. Every
//! peer-provided string goes through [`safe`].

use std::fmt::Write as _;

use carrova_core::session::{Role, SessionId};
use carrova_core::{CoreError, Hint};
use serde_json::json;

use carrova_view::lan::{CodeView, InventoryView, PauseView, SummaryView};

use super::text::{action_counts, human_size, safe};

/// The pairing code, framed so it stands out, with what to do with it.
/// Written to the terminal only (stderr), once per code.
pub fn code_banner(v: &CodeView) -> String {
    let label = "Pairing code:";
    let inner = format!("   {label}  {}   ", v.code);
    let width = inner.chars().count();
    let bar = "\u{2500}".repeat(width);
    let blank = " ".repeat(width);
    let mut s = String::new();
    if v.renewed {
        s.push_str("\nNew code (the previous one can no longer be used):\n");
    }
    let _ = write!(
        s,
        "\n  \u{250c}{bar}\u{2510}\n  \u{2502}{blank}\u{2502}\n  \u{2502}{inner}\u{2502}\n  \u{2502}{blank}\u{2502}\n  \u{2514}{bar}\u{2518}\n\n"
    );
    s.push_str("On the other machine run `carrova receive` and type this code.\n");
    if let Some(ticket) = &v.fallback {
        let _ = writeln!(
            s,
            "If the other machine can't find this one, type: {ticket}"
        );
    }
    s.push_str("Waiting for the other machine... (Ctrl-C to cancel)");
    s
}

/// The loud warning after a failed pairing attempt (`failures` so far,
/// `remaining` more allowed before the send stops), shown before the user
/// is asked whether to offer a new code.
pub fn pairing_warning(failures: u32, remaining: u32) -> String {
    format!(
        "\n!!! WARNING: Someone entered a wrong code. If that wasn't you, stop now. !!!\n\
         Someone nearby may be trying to guess the code. Wrong codes so far: {failures}; \
         the send stops after {remaining} more."
    )
}

/// The question asked after [`pairing_warning`].
pub const NEW_CODE_QUESTION: &str = "Show a new code?";

/// Why no new code is offered when nobody can be asked.
pub const NEW_CODE_NOT_ASKED: &str = "Not offering a new code: nobody can confirm it here (no terminal or --json); stopping the send.";

/// `--json`: the explicit stdout record carrying the code for scripts.
/// The only place the code appears in machine-readable output.
pub fn pairing_record(v: &CodeView) -> serde_json::Value {
    json!({
        "pairing_code": v.code,
        "ticket": v.fallback,
        "renewed": v.renewed,
    })
}

/// The receiver's inventory (multi-line).
pub fn inventory(v: &InventoryView<'_>) -> String {
    let free = v.free_space.map_or_else(
        || "free space unknown".to_owned(),
        |b| format!("{} free", human_size(b)),
    );
    let name = v
        .device_name
        .as_deref()
        .map_or_else(String::new, |n| format!("{} — ", safe(n)));
    let mut s = format!(
        "Other machine: {name}{}, Carrova {}, {free}",
        v.os,
        safe(v.carrova_version)
    );
    if v.apps.is_empty() {
        s.push_str("\n  No known apps detected there.");
    } else {
        let apps: Vec<_> = v.apps.iter().map(|a| safe(a)).collect();
        let _ = write!(s, "\n  Apps detected there: {}", apps.join(", "));
    }
    s
}

/// The receiver's plan, item by item, in the plan view's style
/// (multi-line).
pub fn plan_summary(v: &SummaryView<'_>) -> String {
    let mut s = String::from("Plan on the other machine (it decides and approves):");
    let keys: Vec<_> = v.items.iter().map(|i| safe(i.key)).collect();
    let w = keys.iter().map(|k| k.len()).max().unwrap_or(0);
    for (item, key) in v.items.iter().zip(&keys) {
        let _ = write!(s, "\n  {key:<w$}  {}", action_counts(&item.counts));
        for c in item.conflicts {
            let _ = write!(s, "\n    conflict {}:{}", c.root, safe(&c.rel));
        }
        if item.hidden_conflicts > 0 {
            let _ = write!(
                s,
                "\n    ... and {} more conflict(s)",
                item.hidden_conflicts
            );
        }
    }
    if v.items.is_empty() {
        s.push_str("\n  nothing selected");
    }
    let _ = write!(s, "\nConflicts: {}", v.conflicts);
    if v.truncated {
        s.push_str(" (-v lists every conflict)");
    }
    s
}

/// The receiver's pause notice.
pub fn pause(v: &PauseView<'_>) -> String {
    let resume = v.session.map_or_else(
        || "carrova receive --resume".to_owned(),
        |id| format!("carrova receive --resume {}", safe(&id.0)),
    );
    format!(
        "Paused: need {} more free on {} ({} free). Free up space; transfer resumes \
         automatically. Ctrl-C to stop (resume later with `{resume}`)",
        human_size(v.shortfall),
        safe(v.fs),
        human_size(v.available),
    )
}

/// What the user can do after a LAN session error, if anything specific:
/// the CLI's wording of the engine's [`Hint`] (the resume command for an
/// unreachable peer, the items to deselect when the sender could not
/// supply them). `None` for errors without LAN advice. `id` is the session
/// if known (a `--resume` without an id may fail before naming it).
pub fn recovery_hint(err: &CoreError, id: Option<&SessionId>, role: Role) -> Option<String> {
    let command = match role {
        Role::Export => "send",
        Role::Import => "receive",
    };
    let (session, arg) = match id {
        Some(id) => {
            let id = safe(&id.0);
            (format!("session {id}"), format!(" {id}"))
        }
        None => ("the session".to_owned(), String::new()),
    };
    match err.class().hint? {
        Hint::ResumeWhenBack => Some(format!(
            "lost the other machine; {session} is kept: run `carrova {command} --resume{arg}` \
             when both machines are back"
        )),
        Hint::ResumeAfterPeerCancel => Some(format!(
            "the other machine cancelled; {session} is kept: run `carrova {command} \
             --resume{arg}` once it resumes, or `carrova sessions dismiss{arg}` to drop it"
        )),
        Hint::PeerEnded => Some("the other machine ended the session".to_owned()),
        Hint::DeselectUnsupplied => {
            let items: Vec<_> = err
                .unsupplied_items()
                .unwrap_or_default()
                .iter()
                .map(|i| safe(i))
                .collect();
            Some(format!(
                "nothing was applied: the sender could not supply {}; run `carrova receive \
                 --resume{arg}` and deselect them (pass --item for each item you still want), \
                 or retry once the files are readable on the sender",
                items.join(", ")
            ))
        }
        Hint::CheckCode
        | Hint::ResolveBlockers
        | Hint::NothingChanged
        | Hint::ManualAttention
        | Hint::CheckPassphrase => None,
    }
}

#[cfg(test)]
#[path = "../../tests/render/lan/unit.rs"]
mod tests;
