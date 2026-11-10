//! Sending side: scan, selection, archive export and LAN send.

use std::path::PathBuf;

use carrova_core::{
    CoreError, Exporter, KdfStrength, Preset, ScanOptions, Selection, SelectionEdit, SendReport,
    SessionId, WriteOptions,
};
use carrova_view::ipc::ScanReply;
use carrova_view::passphrase::{self, PassphraseStrength, PassphraseUse, archive_passphrase};
use zeroize::Zeroizing;

use super::task_ended;
use crate::command_error::CommandError;
use crate::pairing::BridgePrompt;
use crate::registry::{Live, Sending};
use crate::state::AppState;
use crate::worker::Worker;

/// The exporter of a session still selecting.
fn selecting(sending: &mut Sending) -> Result<&mut Exporter, CommandError> {
    match sending {
        Sending::Selecting(exporter) => Ok(exporter),
        Sending::Resumed(_) | Sending::Ended => {
            Err(CommandError::no_session("this session is past selection"))
        }
    }
}

/// `exporter`'s scan with `selection`.
fn reply(exporter: &Exporter, selection: Selection) -> ScanReply {
    ScanReply::new(exporter.id().clone(), exporter.scan().clone(), selection)
}

/// Hand `selection` to the engine and reply with it. A selection with
/// issues is refused by the engine (export and send then wait for a
/// usable one) but still shown, issues and all.
fn select_into(exporter: &mut Exporter, selection: Selection) -> Result<ScanReply, CommandError> {
    match exporter.select(selection.clone()) {
        Ok(()) | Err(CoreError::Selection(_)) => Ok(reply(exporter, selection)),
        Err(other) => Err(other.into()),
    }
}

/// Scan this machine: a new export session holding the suggested
/// selection.
///
/// # Errors
///
/// Recipes unavailable, or the scan failed.
pub async fn scan(state: &AppState) -> Result<ScanReply, CommandError> {
    let shared = state.clone();
    let (worker, (id, cancel, result)) = Worker::open("export", move || {
        let sink = shared.unbound_sink();
        let mut exporter = Exporter::start(shared.engine(sink.clone())?, ScanOptions::default())?;
        sink.bind(exporter.id());
        let suggested = exporter.suggest(Preset::default());
        let result = select_into(&mut exporter, suggested)?;
        let opened = (exporter.id().clone(), exporter.canceller(), result);
        Ok::<_, CommandError>((Sending::Selecting(Box::new(exporter)), opened))
    })
    .await?;
    state.sessions.insert(id, Live::Sending(worker), cancel);
    Ok(result)
}

/// The selection `preset` suggests. The session keeps its current
/// selection until [`select`].
///
/// # Errors
///
/// No such session still selecting.
pub async fn suggest(
    state: &AppState,
    id: &SessionId,
    preset: Preset,
) -> Result<ScanReply, CommandError> {
    state
        .sessions
        .sending(id)?
        .call(move |sending| {
            let exporter = selecting(sending)?;
            Ok(reply(exporter, exporter.suggest(preset)))
        })
        .await?
}

/// Apply the user's `edits` to `selection` (the engine's rules,
/// [`Selection::apply_all`]) and hand the result to the engine; replies
/// with it, issues and all.
///
/// # Errors
///
/// No such session still selecting, or an edit names an unknown item
/// (nothing changes).
pub async fn select(
    state: &AppState,
    id: &SessionId,
    mut selection: Selection,
    edits: Vec<SelectionEdit>,
) -> Result<ScanReply, CommandError> {
    selection.apply_all(edits).map_err(CoreError::from)?;
    state
        .sessions
        .sending(id)?
        .call(move |sending| select_into(selecting(sending)?, selection))
        .await?
}

/// How strong `passphrase` is ([`carrova_view::passphrase::strength`],
/// the rule export enforces), for the live meter. The input is wiped
/// afterwards.
#[must_use]
pub fn passphrase_strength(passphrase: String) -> PassphraseStrength {
    passphrase::strength(&Zeroizing::new(passphrase))
}

/// Write the encrypted archive to `path`, its passphrase protected at
/// the user's `strength` (the session ends `Done`).
///
/// # Errors
///
/// Too weak a passphrase, unusable selection, cancel, or a write failure.
pub async fn export_archive(
    state: &AppState,
    id: &SessionId,
    path: PathBuf,
    passphrase: String,
    strength: KdfStrength,
) -> Result<carrova_core::ArchiveSummary, CommandError> {
    let passphrase = archive_passphrase(passphrase, PassphraseUse::NewArchive)?;
    let options = WriteOptions {
        kdf: (state.config.kdf)(strength),
    };
    let worker = state.sessions.sending(id)?;
    let result = worker
        .call(move |sending| {
            let (summary, _checklist) = selecting(sending)?.export(&path, &passphrase, options)?;
            Ok(summary)
        })
        .await
        .and_then(|r| r);
    task_ended(state, id, result)
}

/// Prepare and run a send from a selecting `exporter`. It stays selecting
/// when the send ended before any receiver paired, or never started
/// because the engine refuses the selection.
fn send_selected(
    sending: &mut Sending,
    mut exporter: Box<Exporter>,
    prompt: &mut BridgePrompt,
) -> Result<SendReport, CommandError> {
    let result = exporter.prepare_send().and_then(|h| h.run(prompt));
    let still_selecting = matches!(result, Err(CoreError::Selection(_)))
        || exporter
            .reopen_after_send()
            .inspect_err(|e| tracing::warn!(error = %e, "cannot reopen the send"))
            .unwrap_or(false);
    if still_selecting {
        *sending = Sending::Selecting(exporter);
    }
    Ok(result?)
}

/// Run a LAN send: prepare (capture, listen) a selecting session or take
/// a resumed one, then serve the receiver until it finishes. Codes and
/// "new code?" questions arrive as events. A send that ended before any
/// receiver paired (cancelled, or a new code declined) leaves the session
/// selecting: running it again offers a fresh code.
///
/// # Errors
///
/// Unusable selection, pairing stopped, peer lost or cancelled, cancel.
pub async fn run_send(state: &AppState, id: &SessionId) -> Result<SendReport, CommandError> {
    let mut prompt = BridgePrompt::new(
        id.clone(),
        state.out.clone(),
        state.answers.clone(),
        state.config.answer_timeout,
    );
    let worker = state.sessions.sending(id)?;
    let result = worker
        .call(
            move |sending| match std::mem::replace(sending, Sending::Ended) {
                Sending::Selecting(exporter) => send_selected(sending, exporter, &mut prompt),
                Sending::Resumed(handle) => Ok(handle.run(&mut prompt)?),
                Sending::Ended => Err(CommandError::no_session("this send has already run")),
            },
        )
        .await
        .and_then(|r| r);
    task_ended(state, id, result)
}

/// Answer a "show a new code?" question of a running send.
///
/// # Errors
///
/// No question is open for the session.
pub fn answer_pairing(
    state: &AppState,
    id: &SessionId,
    offer_new_code: bool,
) -> Result<(), CommandError> {
    state
        .answers
        .answer(id, offer_new_code)
        .map_err(|_| CommandError::no_session("no pairing question is open for this session"))
}
