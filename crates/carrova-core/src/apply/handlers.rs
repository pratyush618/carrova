//! Planned handlers: back up the declared paths (copies, journalled), run
//! the handler, fingerprint its result ([`Record::HandlerDone`]).

use std::path::{Path, PathBuf};

use carrova_platform::{FileKind, SemanticRoot};

use super::dirs::not_a_dir_ancestor;
use super::{Applier, CREATED_DIR_MODE, safe_rel, unsafe_path};
use crate::error::Result;
use crate::fsx;
use crate::journal::{self, Record};
use crate::key::RootedPath;
use crate::plan::PlannedHandler;
use crate::session::Event;
use crate::stage::Staged;

impl Applier<'_> {
    pub(super) fn run_handler(&mut self, h: &PlannedHandler, staged: &Staged) -> Result<()> {
        for d in crate::handlers::required_dirs(self.p, h.handler)? {
            self.ensure_handler_dir(&d)?;
        }
        let files = self.backup_handler_paths(h)?;
        let arts: Vec<(String, &Path)> = staged
            .artifacts
            .iter()
            .filter(|(id, _)| id.item == h.item)
            .map(|(id, p)| (id.name.clone(), p.as_path()))
            .collect();
        crate::handlers::import(self.p, h.handler, &arts, h.import_trust)?;
        let after = files
            .into_iter()
            .map(|(abs, _)| journal::fingerprint(self.p, &abs).map(|fp| (abs, fp)))
            .collect::<Result<Vec<_>>>()?;
        self.journal.append(&Record::HandlerDone {
            item: h.item.clone(),
            after,
        })?;
        self.report.handlers_run.push(h.item.clone());
        self.sink.emit(Event::ActionApplied {
            item: h.item.clone(),
            target: format!("handler:{}", h.handler),
            action: "handler".into(),
        });
        Ok(())
    }

    /// A directory a handler writes into (e.g. `GNUPGHOME`). If this apply
    /// creates it, it is journalled so rollback removes it again (if empty)
    /// or quarantines it (files the tool or the user put there later).
    fn ensure_handler_dir(&mut self, dir: &Path) -> Result<()> {
        let home = self.p.resolve_root(SemanticRoot::Home)?;
        let rel = dir
            .strip_prefix(&home)
            .ok()
            .and_then(|r| r.to_str())
            .ok_or_else(|| unsafe_path(dir.display().to_string(), "outside home"))
            .and_then(safe_rel)?;
        self.ensure_root(&home)?;
        self.ensure_parents(&home, &rel)?;
        match fsx::lstat(self.p, dir)? {
            Some(m) if m.kind == FileKind::Dir => {
                if self.created_dirs.contains(dir) {
                    self.journal
                        .append(&Record::HandlerDir { abs: dir.into() })?;
                }
                Ok(())
            }
            Some(_) => Err(unsafe_path(
                dir.display().to_string(),
                "handler directory is a symlink or not a directory",
            )),
            None => {
                self.journal
                    .append(&Record::HandlerDir { abs: dir.into() })?;
                self.p.create_dir(dir, Some(CREATED_DIR_MODE))?;
                self.created_dirs.insert(dir.to_path_buf());
                Ok(())
            }
        }
    }

    /// Copy each declared path that exists into the backup store (via a
    /// `.partial` name renamed when complete), journalled first. Returns
    /// the journalled `(path, backup)` list.
    fn backup_handler_paths(
        &mut self,
        h: &PlannedHandler,
    ) -> Result<Vec<(PathBuf, Option<PathBuf>)>> {
        let mut files = Vec::new();
        for (root, rel) in &h.backup {
            let rel_p = safe_rel(rel)?;
            let root_p = self.p.resolve_root(*root)?;
            let abs = root_p.join(&rel_p);
            if !self.ancestors_safe(&root_p, &rel_p)? {
                files.push((abs, None));
                continue;
            }
            match fsx::lstat(self.p, &abs)? {
                None => files.push((abs, None)),
                Some(m) if m.kind == FileKind::Other => {
                    return Err(unsafe_path(
                        RootedPath::new(*root, rel).to_string(),
                        "not a regular file",
                    ));
                }
                Some(_) => {
                    let b = self.next_backup();
                    files.push((abs, Some(b)));
                }
            }
        }
        self.journal.append(&Record::HandlerBackup {
            item: h.item.clone(),
            files: files.clone(),
        })?;
        for (abs, b) in &files {
            if let Some(b) = b {
                let partial = fsx::partial_of(b);
                fsx::copy_tree(self.p, abs, &partial)?;
                self.p.rename_noreplace(&partial, b)?;
            }
        }
        self.p.sync_dir(&self.backup_dir)?;
        Ok(files)
    }

    /// `Ok(false)` if an ancestor is missing; error on a symlink/non-dir.
    fn ancestors_safe(&self, root: &Path, rel: &Path) -> Result<bool> {
        let Some(parent) = rel.parent() else {
            return Ok(true);
        };
        let mut cur = root.to_path_buf();
        for c in parent.components() {
            cur.push(c);
            match fsx::lstat(self.p, &cur)? {
                Some(m) if m.kind == FileKind::Dir => {}
                Some(_) => return Err(not_a_dir_ancestor(&cur)),
                None => return Ok(false),
            }
        }
        Ok(true)
    }
}
