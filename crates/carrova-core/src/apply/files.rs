//! File actions (the fast path of the module docs): prepare → one journal
//! fsync per batch → backup move + rename into place → one `sync_dir` per
//! directory; the copy path across filesystems.

use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;
use std::time::SystemTime;

use carrova_manifest::{Blake3Hash, EntryKind, Manifest};
use carrova_platform::FileKind;

use super::Applier;
use crate::error::{CoreError, Result};
use crate::fsx;
use crate::journal::Record;
use crate::key::{ItemKey, RootedPath};
use crate::plan::{Action, PlannedAction};
use crate::stage::{STAGED_MODE, Staged};

/// Archive mode assumed for a file entry without one.
const MODELESS_FILE_MODE: u32 = 0o600;

/// `(item key, entry index)` → (BLAKE3, size) of every file entry in a
/// manifest: what staging verified, and what rollback may later remove.
pub(super) struct FileDigests(HashMap<(ItemKey, usize), (Blake3Hash, u64)>);

impl FileDigests {
    pub(super) fn of(manifest: &Manifest) -> Self {
        let mut out = HashMap::new();
        for item in &manifest.items {
            let key = ItemKey::of(item);
            for (i, e) in item.entries.iter().enumerate() {
                if let EntryKind::File { size, hash, .. } = &e.kind {
                    out.insert((key.clone(), i), (*hash, *size));
                }
            }
        }
        Self(out)
    }
}

/// A file action ready to execute: staged file prepared (final mode +
/// mtime), intents buffered in the journal.
pub(super) struct PreparedFile<'a> {
    action: &'a PlannedAction,
    label: String,
    abs: PathBuf,
    /// Staged file (verified, final mode + mtime).
    src: PathBuf,
    hash: Blake3Hash,
    mode: u32,
    mtime: Option<SystemTime>,
    /// Where the existing target is moved first (replace only).
    backup: Option<PathBuf>,
}

/// One entry of a batch, executed in plan order.
pub(super) enum Pending<'a> {
    /// Skip / identical: nothing to do but report it.
    Unchanged(&'a PlannedAction),
    File(PreparedFile<'a>),
}

impl Applier<'_> {
    /// Step 1 of a file action: validate the target, check the staged file
    /// against the plan and manifest, give it its final mode and mtime, and
    /// buffer the intents (backup, then placement).
    pub(super) fn prepare_file<'p>(
        &mut self,
        a: &'p PlannedAction,
        staged: &Staged,
    ) -> Result<PreparedFile<'p>> {
        let (abs, label) = self.target(a)?;
        let key = (a.item.clone(), a.entry_index);
        let unverified = |why: &str| CoreError::Verification(format!("{label} ({why})"));
        let src = staged
            .files
            .get(&key)
            .ok_or_else(|| unverified("entry was not staged"))?
            .clone();
        let &(manifest_hash, manifest_size) = self
            .digests
            .0
            .get(&key)
            .ok_or_else(|| unverified("entry is not a file in the manifest"))?;
        if manifest_size != a.size {
            return Err(unverified("manifest size differs from plan"));
        }
        // A plugin's output replaces the manifest's content.
        let (hash, size) = staged
            .rewritten
            .get(&key)
            .copied()
            .unwrap_or((manifest_hash, manifest_size));
        let staged_size = fsx::lstat(self.p, &src)?
            .filter(|m| m.kind == FileKind::File)
            .map(|m| m.size);
        if staged_size != Some(size) {
            return Err(unverified("staged size differs from plan"));
        }
        let mode = self
            .policy
            .final_mode(a.mode.unwrap_or(MODELESS_FILE_MODE), None);
        let mtime = a.mtime_ns.map(carrova_manifest::time_from_unix_nanos);
        // mtime first: setting it needs an fd, opening needs read access,
        // which the final mode may not grant.
        if let Some(t) = mtime {
            self.p.open_read_nofollow(&src)?.set_modified(t)?;
        }
        fsx::set_mode(self.p, &src, mode)?;

        let backup = if matches!(a.action, Action::Replace) {
            self.backup_intent(&abs, &label)?
        } else {
            None
        };
        self.journal.push(&Record::CreatedFile {
            abs: abs.clone(),
            src: src.clone(),
            hash,
            size,
            backup: backup.clone(),
        })?;
        Ok(PreparedFile {
            action: a,
            label,
            abs,
            src,
            hash,
            mode,
            mtime,
            backup,
        })
    }

    /// Steps 2–4 for everything in `batch`: one journal fsync, then each
    /// entry in order (backup move, rename into place), then one
    /// `sync_dir` per distinct directory, then the events.
    pub(super) fn flush(&mut self, batch: &mut Vec<Pending<'_>>) -> Result<()> {
        if batch.is_empty() {
            return Ok(());
        }
        self.journal.sync()?;
        let mut dirs = BTreeSet::new();
        let mut executed = 0usize;
        for entry in batch.iter() {
            let Pending::File(f) = entry else {
                continue;
            };
            self.failpoint.check(self.done + executed)?;
            if let Some(b) = &f.backup {
                fsx::move_noreplace(self.p, &f.abs, b, &fsx::partial_of(b))?;
                self.report.backed_up.push(f.label.clone());
                dirs.insert(self.backup_dir.clone());
            }
            self.place(f)?;
            let (parent, _) = fsx::split(&f.abs)?;
            dirs.insert(parent.to_path_buf());
            if let Some(staging) = f.src.parent() {
                dirs.insert(staging.to_path_buf());
            }
            executed += 1;
        }
        for d in &dirs {
            self.p.sync_dir(d)?;
        }
        for entry in batch.drain(..) {
            match entry {
                Pending::Unchanged(a) => {
                    self.announce(a, RootedPath::new(a.root, &a.rel).to_string());
                }
                Pending::File(f) => self.finished(f.action, f.label),
            }
        }
        Ok(())
    }

    /// Rename the prepared staged file onto its target; across filesystems
    /// fall back to the copy path ([`Self::place_by_copy`]).
    fn place(&mut self, f: &PreparedFile<'_>) -> Result<()> {
        match self
            .p
            .rename_noreplace(&f.src, &f.abs)
            .map_err(CoreError::from)
        {
            Ok(()) => Ok(()),
            Err(e) if fsx::kind_of(&e) == Some(std::io::ErrorKind::CrossesDevices) => {
                tracing::debug!(target_path = %f.label, "staging on another filesystem; copying");
                self.place_by_copy(f)
            }
            Err(e) => Err(e),
        }
    }

    /// Cross-device placement: copy the staged file to a journalled
    /// sibling temp (final mode + mtime, fsynced), journal the placement
    /// of that temp, rename it into place. The staged file stays (wiped
    /// with staging), so the earlier intent naming it is a no-op on
    /// rollback.
    fn place_by_copy(&mut self, f: &PreparedFile<'_>) -> Result<()> {
        let (parent, name) = fsx::split(&f.abs)?;
        let tmp = parent.join(format!(".{name}.carrova-tmp-{}", self.sid));
        self.journal
            .append(&Record::CreatedTemp { abs: tmp.clone() })?;
        // The staged file already has its final mode, which may not allow
        // reading it back; it is ours.
        fsx::set_mode(self.p, &f.src, STAGED_MODE)?;
        {
            let mut out = self.p.create_new_file(&tmp, Some(f.mode))?;
            let mut inp = self.p.open_read_nofollow(&f.src)?;
            let n = std::io::copy(&mut inp, &mut out)?;
            if n != f.action.size {
                return Err(CoreError::Verification(format!(
                    "{} (staged size differs from plan)",
                    f.label
                )));
            }
            if let Some(t) = f.mtime {
                out.set_modified(t)?;
            }
            out.sync_all()?;
        }
        self.journal.append(&Record::CreatedFile {
            abs: f.abs.clone(),
            src: tmp.clone(),
            hash: f.hash,
            size: f.action.size,
            backup: f.backup.clone(),
        })?;
        self.p.rename_noreplace(&tmp, &f.abs)?;
        Ok(())
    }
}
