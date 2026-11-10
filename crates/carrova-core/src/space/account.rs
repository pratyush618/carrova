//! The per-filesystem tally behind [`super::space_needs`].

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use carrova_platform::{FsId, Platform, SemanticRoot};

use super::{SpaceNeed, required_space};

/// Identity of a filesystem for grouping; a path stands in when the
/// platform cannot tell (counted as its own filesystem: conservative).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum FsKey {
    Id(FsId),
    Unknown(PathBuf),
}

/// Accumulates payload bytes per filesystem.
#[derive(Debug)]
pub(super) struct SpaceAccount<'a> {
    platform: &'a dyn Platform,
    /// State dir filesystem (`None` if the state dir is unknown).
    state: Option<FsKey>,
    /// Filesystem of each target root seen (`None`: root unresolvable).
    roots: BTreeMap<SemanticRoot, Option<(FsKey, PathBuf)>>,
    /// Per filesystem: a path on it (for free space) and its need.
    needs: BTreeMap<FsKey, (PathBuf, SpaceNeed)>,
}

impl<'a> SpaceAccount<'a> {
    /// Start an account; the state dir's filesystem always gets an entry.
    pub(super) fn new(platform: &'a dyn Platform) -> Self {
        let mut acc = Self {
            platform,
            state: None,
            roots: BTreeMap::new(),
            needs: BTreeMap::new(),
        };
        let state = platform.state_dir().ok().and_then(|d| locate(platform, &d));
        if let Some((key, path)) = state {
            acc.entry(key.clone(), &path).state_dir = true;
            acc.state = Some(key);
        }
        acc
    }

    fn entry(&mut self, key: FsKey, path: &Path) -> &mut SpaceNeed {
        &mut self
            .needs
            .entry(key)
            .or_insert_with(|| {
                (
                    path.to_path_buf(),
                    SpaceNeed {
                        state_dir: false,
                        roots: Vec::new(),
                        bytes: 0,
                        needed: 0,
                        available: None,
                    },
                )
            })
            .1
    }

    /// `bytes` staged in the state dir, then moved into `root` (`None`:
    /// no final copy on a root, e.g. a handler artifact).
    pub(super) fn add(&mut self, root: Option<SemanticRoot>, bytes: u64) {
        if let Some(key) = self.state.clone()
            && let Some((_, n)) = self.needs.get_mut(&key)
        {
            n.bytes = n.bytes.saturating_add(bytes);
        }
        let Some(root) = root else { return };
        let platform = self.platform;
        let target = self
            .roots
            .entry(root)
            .or_insert_with(|| {
                platform
                    .resolve_root(root)
                    .ok()
                    .and_then(|abs| locate(platform, &abs))
            })
            .clone();
        let Some((key, path)) = target else { return };
        let same = self.state.as_ref() == Some(&key);
        let n = self.entry(key, &path);
        if !n.roots.contains(&root) {
            n.roots.push(root);
        }
        if !same {
            n.bytes = n.bytes.saturating_add(bytes);
        }
    }

    /// Final needs with margins and free space, state-dir filesystem first.
    pub(super) fn finish(self) -> Vec<SpaceNeed> {
        let platform = self.platform;
        let mut out: Vec<SpaceNeed> = self
            .needs
            .into_values()
            .filter(|(_, n)| n.bytes > 0 || n.state_dir)
            .map(|(path, mut n)| {
                n.needed = required_space(n.bytes);
                n.available = platform.free_space(&path).ok();
                n.roots.sort_unstable();
                n
            })
            .collect();
        out.sort_by_key(|n| !n.state_dir);
        out
    }
}

/// Filesystem of `path` (or of its nearest existing ancestor: roots may
/// not exist yet) and the existing path used to query it.
fn locate(platform: &dyn Platform, path: &Path) -> Option<(FsKey, PathBuf)> {
    let existing = nearest_existing(platform, path)?;
    let key = platform
        .fs_id(&existing)
        .map_or_else(|_| FsKey::Unknown(existing.clone()), FsKey::Id);
    Some((key, existing))
}

/// `path` or its nearest ancestor that exists (lstat), if any.
pub(crate) fn nearest_existing(platform: &dyn Platform, path: &Path) -> Option<PathBuf> {
    path.ancestors()
        .find(|p| platform.metadata(p).is_ok())
        .map(Path::to_path_buf)
}
