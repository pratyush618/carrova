//! Sender session: scan → select → capture → archive file (here) or LAN
//! send (`send.rs`).

use std::path::Path;

use carrova_archive::{ArchiveSummary, WriteOptions};
use carrova_crypto::Passphrase;

use super::Engine;
use super::redact::Patterns;
use super::session::{Session, absolute};
use crate::cancel::Cancel;
use crate::checklist::Checklist;
use crate::error::{CoreError, Result};
use crate::key::ItemKey;
use crate::select::{self, Preset, Selection, SelectionIssue};
use crate::session::{Event, Role, SessionId, State};
use crate::store::{SELECTION_FILE, SessionRecord, Source};
use crate::{ScanOptions, ScanReport, capture, export};

/// Sender session. Archive exports are not resumable (an interrupted export
/// is simply re-run); LAN sends are ([`Self::resume_send`]).
#[derive(Debug)]
pub struct Exporter {
    pub(super) s: Session,
    scan: ScanReport,
    pub(super) selection: Chosen,
    /// Stops [`Self::export`] / [`Self::prepare_send`] (and the send it
    /// prepares); see [`crate::cancel`].
    pub(super) cancel: Cancel,
}

/// What the user last selected ([`Exporter::select`]).
#[derive(Debug)]
pub(super) enum Chosen {
    /// Nothing yet.
    Nothing,
    /// A usable selection: export and send capture it.
    Usable(Selection),
    /// The last selection was refused for these issues: export and send
    /// are refused until a usable one is selected, so nothing the user
    /// moved away from is ever captured.
    Refused(Vec<SelectionIssue>),
}

impl Chosen {
    /// The usable selection, if any.
    pub(super) fn usable(&self) -> Option<&Selection> {
        match self {
            Self::Usable(selection) => Some(selection),
            Self::Nothing | Self::Refused(_) => None,
        }
    }
}

impl Exporter {
    /// New export session: scans and persists it (state `Scanned`).
    ///
    /// Not cancellable; see [`Self::start_with`].
    pub fn start(engine: Engine, scan_opts: ScanOptions) -> Result<Self> {
        Self::start_with(engine, scan_opts, Cancel::default())
    }

    /// [`Self::start`], stopped by `cancel` (which becomes this exporter's
    /// [`Self::canceller`]) from another thread.
    ///
    /// The scan walks every unrecognised directory, which can take long on
    /// a big home; a cancel ends it with [`CoreError::Cancelled`] at the
    /// next recipe or directory entry. The session is created only after
    /// the scan, so a cancelled start leaves nothing behind.
    pub fn start_with(engine: Engine, scan_opts: ScanOptions, cancel: Cancel) -> Result<Self> {
        let scan = crate::scan::scan_with(&engine.recipes, &*engine.platform, scan_opts, &cancel)?;
        let s = Session::create(engine, Role::Export, State::Scanned, None)?;
        Ok(Self {
            s,
            scan,
            selection: Chosen::Nothing,
            cancel,
        })
    }

    /// Cancels this session's running (or next) blocking call from another
    /// thread: take it before calling [`Self::export`] or
    /// [`Self::prepare_send`]. The prepared [`super::SendHandle`] shares it.
    #[must_use]
    pub fn canceller(&self) -> Cancel {
        self.cancel.clone()
    }

    /// Session id.
    #[must_use]
    pub fn id(&self) -> &SessionId {
        self.s.id()
    }

    /// Current state.
    #[must_use]
    pub fn state(&self) -> State {
        self.s.record.state
    }

    /// Persisted record (state, source, checklist, error).
    #[must_use]
    pub fn record(&self) -> &SessionRecord {
        &self.s.record
    }

    /// Scan result the selection is built from.
    #[must_use]
    pub fn scan(&self) -> &ScanReport {
        &self.scan
    }

    /// Selection fixed by [`Self::select`]; `None` before, and after a
    /// refused one.
    #[must_use]
    pub fn selection(&self) -> Option<&Selection> {
        self.selection.usable()
    }

    /// Suggested selection for `preset`; the user edits it before `select`.
    #[must_use]
    pub fn suggest(&self, preset: Preset) -> Selection {
        Selection::from_scan(&self.scan, preset)
    }

    /// Fix the user's selection (state `Selected`, `selection.json`).
    /// May be called again to change it. A selection with
    /// [`Selection::issues`] is refused with [`CoreError::Selection`] and
    /// replaces the earlier one: [`Self::export`] and
    /// [`Self::prepare_send`] refuse until a usable selection is selected.
    pub fn select(&mut self, selection: Selection) -> Result<()> {
        let from = self.s.record.state;
        if !from.can_transition(State::Selected) {
            return Err(CoreError::InvalidTransition {
                from,
                to: State::Selected,
            });
        }
        let issues = selection.issues();
        if !issues.is_empty() {
            let refused = select::refusal(&issues);
            self.selection = Chosen::Refused(issues);
            return Err(refused);
        }
        self.s
            .add_redactions(Patterns::new(self.s.platform()).selection(&selection))?;
        self.s.write_json(SELECTION_FILE, &selection)?;
        self.s.transition(State::Selected)?;
        self.selection = Chosen::Usable(selection);
        Ok(())
    }

    /// The selection to capture: only a usable one, from `Selected` (to
    /// `Transferring`).
    pub(super) fn selected(&self) -> Result<Selection> {
        match (&self.selection, self.s.record.state) {
            (Chosen::Usable(sel), State::Selected) => Ok(sel.clone()),
            (Chosen::Refused(issues), State::Selected) => Err(select::refusal(issues)),
            (_, from) => Err(CoreError::InvalidTransition {
                from,
                to: State::Transferring,
            }),
        }
    }

    /// Capture + write the archive (states `Transferring` → `Done`).
    /// The checklist lists captured item keys as moved and failed items as
    /// skipped; `apps_to_install` is empty on export. A cancel
    /// ([`Self::canceller`]) removes the partial archive and fails the
    /// session with [`CoreError::Cancelled`].
    pub fn export(
        &mut self,
        path: &Path,
        pass: &Passphrase,
        opts: WriteOptions,
    ) -> Result<(ArchiveSummary, Checklist)> {
        let selection = self.selected()?;
        self.s.record.source = Some(Source::Archive {
            path: absolute(path),
            fingerprint: None,
        });
        self.s.transition(State::Transferring)?;
        let cancel = &self.cancel;
        self.s
            .failing(|s| write_archive(s, &selection, path, pass, opts, cancel))
    }
}

/// Capture `selection`, write it to `path` and finish the session.
fn write_archive(
    s: &mut Session,
    selection: &Selection,
    path: &Path,
    pass: &Passphrase,
    opts: WriteOptions,
    cancel: &Cancel,
) -> Result<(ArchiveSummary, Checklist)> {
    let platform = s.platform();
    let cap = capture::capture(platform, &s.engine.recipes, selection, s.sink(), cancel)?;
    let summary = export::write(platform, &cap, path, pass, opts, s.sink(), cancel)?;
    let checklist = Checklist {
        moved: cap.manifest.items.iter().map(ItemKey::of).collect(),
        skipped: cap.failed,
        apps_to_install: Vec::new(),
        rollback_available: false,
    };
    s.record.checklist = Some(checklist.clone());
    s.record.bytes_moved = Some(summary.bytes);
    s.transition(State::Done)?;
    s.emit(Event::Finished {
        checklist: checklist.clone(),
    });
    Ok((summary, checklist))
}
