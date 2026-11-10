//! Apply (receiver side): journalled, reversible swap of staged data into
//! place. Parents are created/walked with lstat; a symlink component
//! aborts. A journal record is durable *before* the step it describes.
//!
//! # Files (fast path)
//!
//! File actions run in batches of up to [`APPLY_BATCH`] (`files`):
//! 1. **Prepare** each file: check the staged file against the plan, set
//!    its final mode (the plan's `ModePolicy`) and mtime *on the staged file*,
//!    and buffer its intents — [`Record::BackedUp`] if an existing target
//!    is replaced, then [`Record::CreatedFile`] (hash and size from the
//!    manifest, or [`Staged::rewritten`] for a plugin's output: staging
//!    already verified or produced those bytes, they are not hashed
//!    again).
//! 2. **One journal fsync** for the whole batch.
//! 3. **Execute** each file in order: move the existing target to the
//!    session `backup/` (`rename_noreplace`; copy+fsync on EXDEV), then
//!    `rename_noreplace` the staged file onto the target — no copy.
//! 4. **One `sync_dir`** per distinct directory touched (targets' parents,
//!    `backup/`, the staging directory).
//!
//! **Cross-device fallback:** if the staged file is on another filesystem
//! than the target (`rename` fails with `CrossesDevices`), that file goes
//! the copy path instead, transparently: journalled sibling temp
//! `.<name>.carrova-tmp-<sid>` (create-new, nofollow, final mode + mtime),
//! fsync, a second `CreatedFile` intent naming the temp, rename into place.
//! Only the bytes of that file are written twice.
//!
//! Requires the staged files' contents to be durable already (staging
//! fsyncs/checkpoints them) and each staged path to serve one action. A
//! crash after a rename leaves the target with the staged file's mode at
//! worst (`600`, never looser), and rollback still recognises it by hash.
//!
//! Directories, symlinks and mode changes of existing directories run one
//! by one between batches (each with its own journal record), in plan order
//! sorted by depth (`dirs`). Handlers back up their declared paths before
//! running and their result is fingerprinted afterwards
//! ([`Record::HandlerDone`], `handlers`). Any error → automatic rollback
//! ([`crate::rollback`]), reported as [`crate::CoreError::RolledBack`].
//!
//! Requirements on the caller: `session_dir` exists and holds no journal
//! yet (one apply per session); the plan is approved and was computed
//! against the current filesystem from `manifest` (apply re-checks
//! confinement and never clobbers a target the plan did not mark
//! `Replace`).
//!
//! Permissions are never loosened (defence in depth; the plan is the
//! authority): every mode apply sets goes through the plan's
//! `ModePolicy` again, so an existing directory's mode only ever loses
//! bits. Semantic roots must be real directories
//! (or missing): a symlinked root is refused, like [`crate::plan`] does.

mod backup;
mod dirs;
mod failpoint;
mod files;
mod handlers;

pub use failpoint::Failpoint;

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};

use carrova_platform::Platform;

use crate::error::{CoreError, Result};
use crate::journal::{BACKUP_DIR, JOURNAL_FILE, JOURNAL_VERSION, Journal, Record};
use crate::key::{ItemKey, RootedPath};
use crate::mode::ModePolicy;
use crate::plan::{ObjectKind, Plan, PlannedAction};
use crate::session::{Event, EventSink, Phase, ProgressMeter, SessionId};
use crate::stage::Staged;
use files::{FileDigests, Pending};

/// Most file actions whose journal intents share one fsync (and whose
/// directories share one `sync_dir` each).
pub const APPLY_BATCH: usize = 1024;

/// Mode of directories apply creates (roots, parents, `backup/`).
const CREATED_DIR_MODE: u32 = 0o700;

/// Outcome of a successful apply.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplyReport {
    /// Targets written (`root:rel`).
    pub written: Vec<String>,
    /// Existing targets moved to the rollback store.
    pub backed_up: Vec<String>,
    /// Handlers run (item keys).
    pub handlers_run: Vec<ItemKey>,
}

/// Apply an **approved** plan computed from `manifest`. `session_dir`
/// holds `journal.cbor` + `backup/`.
pub fn apply(
    platform: &dyn Platform,
    plan: &Plan,
    manifest: &carrova_manifest::Manifest,
    staged: &Staged,
    session_dir: &Path,
    sink: &dyn EventSink,
    failpoint: Failpoint,
) -> Result<ApplyReport> {
    plan.ensure_unblocked()?;
    if !SessionId::is_valid(&plan.session.0) {
        return Err(CoreError::InvalidPlanSession);
    }
    // Nothing has been written before the journal exists, so a failure here
    // needs no rollback. An existing journal means a previous apply's
    // rollback point is still there: refuse rather than mix them.
    let journal = Journal::create(platform, &session_dir.join(JOURNAL_FILE))?;
    let mut ap = Applier {
        p: platform,
        journal,
        backup_dir: session_dir.join(BACKUP_DIR),
        sid: plan.session.0.clone(),
        backup_seq: 0,
        created_dirs: HashSet::new(),
        report: ApplyReport::default(),
        sink,
        policy: ModePolicy::of(&plan.options),
        digests: FileDigests::of(manifest),
        done: 0,
        progress: ProgressMeter::new(
            sink,
            Phase::Apply,
            plan.bytes_to_write,
            ProgressMeter::EVERY_UPDATE,
        ),
        failpoint,
    };
    match ap.run(plan, staged) {
        Ok(()) => Ok(ap.report),
        Err(cause) => {
            drop(ap); // close the journal before rollback reopens it
            Err(auto_rollback(platform, session_dir, sink, &cause))
        }
    }
}

/// Roll back after `cause`; the error to report.
fn auto_rollback(
    platform: &dyn Platform,
    session_dir: &Path,
    sink: &dyn EventSink,
    cause: &CoreError,
) -> CoreError {
    tracing::warn!(error = %cause, "apply failed; rolling back");
    let cause = cause.to_string();
    match crate::rollback::rollback(platform, session_dir, sink) {
        Ok(r) if r.failed.is_empty() => CoreError::RolledBack { cause },
        Ok(r) => CoreError::RollbackFailed {
            cause,
            rollback: r.failure_summary(),
        },
        Err(e) => CoreError::RollbackFailed {
            cause,
            rollback: e.to_string(),
        },
    }
}

/// Validate a relative path: non-empty, only normal components.
fn safe_rel(rel: &str) -> Result<PathBuf> {
    let p = Path::new(rel);
    let ok = !rel.is_empty() && p.components().all(|c| matches!(c, Component::Normal(_)));
    if !ok {
        return Err(unsafe_path(rel, "not a plain relative path"));
    }
    Ok(p.to_path_buf())
}

fn unsafe_path(path: impl Into<String>, reason: &str) -> CoreError {
    CoreError::UnsafePath {
        path: path.into(),
        reason: reason.into(),
    }
}

/// One apply run.
struct Applier<'a> {
    p: &'a dyn Platform,
    journal: Journal,
    backup_dir: PathBuf,
    sid: String,
    backup_seq: u64,
    /// Directories this apply created (owned by the session).
    created_dirs: HashSet<PathBuf>,
    report: ApplyReport,
    sink: &'a dyn EventSink,
    /// The plan's mode policy, re-applied to every mode set.
    policy: ModePolicy,
    digests: FileDigests,
    /// Actions completed (failpoint counter).
    done: usize,
    /// File bytes of completed actions.
    progress: ProgressMeter<'a>,
    failpoint: Failpoint,
}

impl Applier<'_> {
    fn run(&mut self, plan: &Plan, staged: &Staged) -> Result<()> {
        self.journal.append(&Record::Begin {
            session: self.sid.clone(),
            version: JOURNAL_VERSION,
        })?;
        crate::fsx::create_dir_or_existing(self.p, &self.backup_dir, CREATED_DIR_MODE)?;

        // Shallower targets first, so planned dirs exist (with their exact
        // mode) before their contents; plan order otherwise (stable sort).
        let mut order: Vec<&PlannedAction> = plan.actions.iter().collect();
        order.sort_by_key(|a| Path::new(a.target_rel()).components().count());

        let mut batch: Vec<Pending<'_>> = Vec::new();
        let mut files = 0usize;
        for a in order {
            if a.action.is_unchanged() {
                batch.push(Pending::Unchanged(a));
                continue;
            }
            match a.kind {
                ObjectKind::File => {
                    batch.push(Pending::File(self.prepare_file(a, staged)?));
                    files += 1;
                    if files == APPLY_BATCH {
                        self.flush(&mut batch)?;
                        files = 0;
                    }
                }
                ObjectKind::Dir | ObjectKind::Symlink => {
                    self.flush(&mut batch)?;
                    files = 0;
                    self.apply_object(a)?;
                }
            }
        }
        self.flush(&mut batch)?;
        self.failpoint.check(self.done)?;

        for h in &plan.handlers {
            self.run_handler(h, staged)?;
        }
        self.journal.append(&Record::Committed)?;
        Ok(())
    }

    /// Validate `a`'s target and make its root and parents exist (walked
    /// with lstat, never through a symlink). Returns `(abs, "root:rel")`.
    fn target(&mut self, a: &PlannedAction) -> Result<(PathBuf, String)> {
        let rel_str = a.target_rel();
        let rel = safe_rel(rel_str)?;
        let root = self.p.resolve_root(a.root)?;
        let abs = root.join(&rel);
        let label = RootedPath::new(a.root, rel_str).to_string();
        if !matches!(a.action, crate::plan::Action::KeepBoth { .. }) && abs != a.abs {
            return Err(unsafe_path(label, "plan target does not match its root"));
        }
        self.ensure_root(&root)?;
        self.ensure_parents(&root, &rel)?;
        Ok((abs, label))
    }

    /// Count, report and announce a completed action.
    fn finished(&mut self, a: &PlannedAction, label: String) {
        tracing::debug!(target_path = %label, action = a.action.as_str(), "applied");
        self.done += 1;
        self.report.written.push(label.clone());
        self.announce(a, label);
        self.progress.add(if a.kind == ObjectKind::File {
            a.size
        } else {
            0
        });
    }

    /// [`Event::ActionApplied`] for `a` at `target`.
    fn announce(&self, a: &PlannedAction, target: String) {
        self.sink.emit(Event::ActionApplied {
            item: a.item.clone(),
            target,
            action: a.action.as_str().into(),
        });
    }
}
