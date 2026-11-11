//! Sender session: scan → select → capture → archive file (here) or LAN
//! send (`send.rs`).

use std::path::{Path, PathBuf};

use carrova_archive::{ArchiveSummary, WriteOptions};
use carrova_crypto::Passphrase;

use super::Engine;
use super::redact::Patterns;
use super::session::{Session, absolute};
use crate::cancel::Cancel;
use crate::checklist::Checklist;
use crate::error::{CoreError, Result};
use crate::key::{ItemKey, ItemTarget, RootedPath};
use crate::secrets::secret_in;
use crate::select::{
    self, PickOutcome, PickRejection, PickResult, Preset, Selection, SelectionIssue,
};
use crate::session::{Event, Role, SessionId, State};
use crate::store::{SELECTION_FILE, SessionRecord, Source};
use crate::{ScanOptions, ScanReport, capture, export};

/// Sender session. Archive exports are not resumable (an interrupted export
/// is simply re-run); LAN sends are ([`Self::resume_send`]).
#[derive(Debug)]
pub struct Exporter {
    pub(super) s: Session,
    scan: ScanReport,
    /// [`Self::scan`] holds the app scan (false only for a Manual start
    /// before [`Self::scan_now`]).
    apps_scanned: bool,
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
        Self::create(engine, scan, true, cancel)
    }

    /// Persist a new session (state `Scanned`) around `scan`.
    fn create(
        engine: Engine,
        scan: ScanReport,
        apps_scanned: bool,
        cancel: Cancel,
    ) -> Result<Self> {
        let s = Session::create(engine, Role::Export, State::Scanned, None)?;
        Ok(Self {
            s,
            scan,
            apps_scanned,
            selection: Chosen::Nothing,
            cancel,
        })
    }

    /// New Manual export session: no app scan (the scan report stays
    /// empty until [`Self::scan_now`]); the user adds picks
    /// ([`Self::pick`]) to an empty selection. `cancel` becomes
    /// [`Self::canceller`].
    pub fn start_manual(engine: Engine, cancel: Cancel) -> Result<Self> {
        // No recipes, no unrecognised walk: only the machine survey (roots).
        let opts = ScanOptions {
            include_undetected: false,
            unrecognised: false,
        };
        let scan = crate::scan::scan_with(&[], &*engine.platform, opts, &cancel)?;
        Self::create(engine, scan, false, cancel)
    }

    /// Run the app scan of a session started by [`Self::start_manual`]
    /// (the first switch to Auto), or return the one already run.
    ///
    /// # Errors
    ///
    /// The scan failed or was cancelled ([`Self::canceller`]).
    pub fn scan_now(&mut self, opts: ScanOptions) -> Result<&ScanReport> {
        if !self.apps_scanned {
            let engine = &self.s.engine;
            self.scan =
                crate::scan::scan_with(&engine.recipes, &*engine.platform, opts, &self.cancel)?;
            self.apps_scanned = true;
        }
        Ok(&self.scan)
    }

    /// Resolve each of `paths` ([`select::resolve_pick`]) and add it to
    /// `selection` ([`Selection::add_pick`]), one outcome per path, in
    /// order.
    #[must_use]
    pub fn pick(&self, selection: &mut Selection, paths: &[PathBuf]) -> Vec<PickOutcome> {
        let platform = self.s.platform();
        paths
            .iter()
            .map(|path| PickOutcome {
                path: path.to_string_lossy().into_owned(),
                result: match select::resolve_pick(platform, path) {
                    Ok(pick) => selection.add_pick(pick),
                    Err(reason) => PickResult::Rejected { reason },
                },
            })
            .collect()
    }

    /// The item key absolute `path` would be picked as (for naming a pick
    /// by path, e.g. the CLI's `--confirm-sensitive PATH`).
    ///
    /// # Errors
    ///
    /// The path cannot be picked.
    pub fn pick_key(&self, path: &Path) -> std::result::Result<ItemKey, PickRejection> {
        let (root, rel) = select::locate_pick(self.s.platform(), path)?;
        Ok(ItemKey::raw(RootedPath::new(root, &rel)))
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

    /// Suggested selection for `preset`, with the plugins loaded here
    /// that export its items ([`Selection::mark_exports`]); the user edits
    /// it before `select`.
    #[must_use]
    pub fn suggest(&self, preset: Preset) -> Selection {
        let mut selection = Selection::from_scan(&self.scan, preset);
        selection.mark_exports(self.s.engine.recipes(), self.s.engine.plugins());
        selection
    }

    /// Fix the user's selection (state `Selected`, `selection.json`).
    /// May be called again to change it. A selection with
    /// [`Selection::issues`] is refused with [`CoreError::Selection`] and
    /// replaces the earlier one: [`Self::export`] and
    /// [`Self::prepare_send`] refuse until a usable selection is selected.
    /// Which items a plugin exports is the engine's call: it is marked
    /// afresh ([`Selection::mark_exports`]) before the check, and so is a
    /// raw pick lying in a secret location ([`crate::secrets::secret_in`]):
    /// sensitive, so it needs its confirmation.
    pub fn select(&mut self, mut selection: Selection) -> Result<()> {
        let from = self.s.record.state;
        if !from.can_transition(State::Selected) {
            return Err(CoreError::InvalidTransition {
                from,
                to: State::Selected,
            });
        }
        selection.mark_exports(self.s.engine.recipes(), self.s.engine.plugins());
        // A raw pick in a secret location is sensitive whatever the front-end says.
        for (key, choice) in &mut selection.items {
            if let Some(ItemTarget::Raw(path)) = key.target() {
                choice.sensitive |= secret_in(path.rel).is_some();
            }
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
    let cap = capture::capture_with_consent(
        platform,
        &s.engine.recipes,
        &s.engine.plugins,
        &selection.confirmed_plugins,
        selection,
        s.sink(),
        cancel,
    )?;
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
