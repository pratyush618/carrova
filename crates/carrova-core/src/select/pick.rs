//! Manual picks: files and folders the user chose by path, carried as raw
//! items (`raw:` keys) at any depth under the most specific semantic root.

use std::io;
use std::path::{Path, PathBuf};

use carrova_manifest::{RelPath, is_unsafe_char};
use carrova_platform::path::normalize_abs;
use carrova_platform::{FileKind, Platform, PlatformError, SemanticRoot};
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use super::Selection;
use crate::key::{ItemKey, ItemTarget, RootedPath};
use crate::secrets::secret_in;

/// What a pick is on disk (never following the pick itself).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum PickKind {
    /// A regular file.
    File,
    /// A folder.
    Dir,
    /// A link that stays inside the semantic roots (moved as a link).
    Symlink,
}

/// What the engine measured of a pick when it was added.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PickFacts {
    /// File, folder or link.
    pub kind: PickKind,
    /// Bytes in regular files.
    pub size: u64,
    /// Regular files.
    pub files: u64,
}

/// A path the user picked, resolved against the semantic roots
/// ([`resolve_pick`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pick {
    /// The most specific semantic root holding it.
    pub root: SemanticRoot,
    /// `/`-separated path under `root`.
    pub rel: String,
    /// What it is and how big.
    pub facts: PickFacts,
    /// It is or holds a secret location ([`crate::secrets::secret_in`]).
    pub sensitive: bool,
}

impl Pick {
    /// The pick as a rooted path.
    #[must_use]
    pub fn path(&self) -> RootedPath<'_> {
        RootedPath::new(self.root, &self.rel)
    }

    /// The pick's item key ([`ItemKey::raw`]).
    #[must_use]
    pub fn key(&self) -> ItemKey {
        ItemKey::raw(self.path())
    }
}

/// Why a path cannot be picked, in the user's words.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PickRejection {
    /// Nothing at that path.
    #[error("it no longer exists")]
    NotFound,
    /// Under no semantic root.
    #[error("it is outside your home and app folders; Carrova only moves files from those")]
    OutsideRoots,
    /// A whole semantic root.
    #[error("it is your whole {}; pick folders inside it", root_words(*root))]
    IsRoot {
        /// The root it is.
        root: SemanticRoot,
    },
    /// Holds another semantic root.
    #[error("it holds your {}; pick folders inside it", root_words(*root))]
    ContainsRoot {
        /// The root it holds.
        root: SemanticRoot,
    },
    /// A folder on its path (below the root) is a link.
    #[error("a folder on its path is a link; pick the real folder instead")]
    LinkedParent,
    /// The pick is a link pointing out of the semantic roots.
    #[error("it is a link to a place outside your home and app folders")]
    SymlinkOutside,
    /// A device, socket, pipe or the like.
    #[error("it is not a regular file or folder")]
    Special,
    /// Its name is not portable text (non-UTF-8, control or bidi
    /// characters, `\`).
    #[error("its name has characters Carrova can't carry")]
    UnsupportedName,
    /// It cannot be read.
    #[error("Carrova can't read it ({reason})")]
    Unreadable {
        /// What went wrong.
        reason: String,
    },
}

/// What adding one pick did to the selection ([`Selection::add_pick`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PickResult {
    /// Added as `item`, replacing the picks inside it.
    Added {
        /// The new item.
        item: ItemKey,
        /// Earlier picks it holds, now removed.
        replaced: Vec<ItemKey>,
    },
    /// Already inside picked folder `into`; nothing added.
    Absorbed {
        /// The pick that holds it.
        into: ItemKey,
    },
    /// Already picked.
    Duplicate {
        /// The existing item.
        item: ItemKey,
    },
    /// Refused.
    Rejected {
        /// Why.
        reason: PickRejection,
    },
}

/// The outcome of one path the user picked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PickOutcome {
    /// The path as given (lossy text).
    pub path: String,
    /// What happened to it.
    pub result: PickResult,
}

/// The most specific semantic root holding absolute `path`, and the
/// `/`-separated path under it.
///
/// # Errors
///
/// The path cannot be picked ([`PickRejection`]).
pub fn locate_pick(
    platform: &dyn Platform,
    path: &Path,
) -> Result<(SemanticRoot, String), PickRejection> {
    locate(platform, path).map(|l| (l.root, l.rel))
}

/// Resolve absolute `path` into a [`Pick`]: located ([`locate_pick`]),
/// checked and measured.
///
/// # Errors
///
/// The path cannot be picked ([`PickRejection`]).
pub fn resolve_pick(platform: &dyn Platform, path: &Path) -> Result<Pick, PickRejection> {
    let Located {
        root,
        rel,
        path,
        roots,
    } = locate(platform, path)?;
    let meta = platform.metadata(&path).map_err(rejection)?;
    let (facts, holds_secret) = match meta.kind {
        FileKind::Other => return Err(PickRejection::Special),
        FileKind::Symlink => {
            check_link(platform, &path, &link_scope(&roots, root))?;
            (facts(PickKind::Symlink, 0, 0), false)
        }
        FileKind::File => {
            platform.open_read_nofollow(&path).map_err(rejection)?;
            (facts(PickKind::File, meta.size, 1), false)
        }
        FileKind::Dir => measure(&path, &rel)?,
    };
    Ok(Pick {
        root,
        sensitive: holds_secret || secret_in(&rel).is_some(),
        rel,
        facts,
    })
}

/// A path checked against the semantic roots ([`locate`]).
struct Located {
    root: SemanticRoot,
    rel: String,
    /// The normalised absolute path.
    path: PathBuf,
    /// Every root that resolved, normalised.
    roots: Vec<(SemanticRoot, PathBuf)>,
}

/// [`locate_pick`], keeping what [`resolve_pick`] needs next.
fn locate(platform: &dyn Platform, path: &Path) -> Result<Located, PickRejection> {
    let path = normalize_abs(path).ok_or(PickRejection::OutsideRoots)?;
    let roots: Vec<(SemanticRoot, PathBuf)> = SemanticRoot::ALL
        .into_iter()
        .filter_map(|r| {
            let dir = platform.resolve_root(r).ok()?;
            normalize_abs(&dir).map(|dir| (r, dir))
        })
        .collect();
    if let Some(&(root, _)) = roots.iter().find(|(_, dir)| *dir == path) {
        return Err(PickRejection::IsRoot { root });
    }
    if let Some(&(root, _)) = roots.iter().find(|(_, dir)| dir.starts_with(&path)) {
        return Err(PickRejection::ContainsRoot { root });
    }
    let (root, base) = roots
        .iter()
        .filter(|(_, dir)| path.starts_with(dir))
        .max_by_key(|(_, dir)| dir.components().count())
        .ok_or(PickRejection::OutsideRoots)?;
    let names = path
        .strip_prefix(base)
        .map_err(|_| PickRejection::OutsideRoots)?
        .iter()
        .map(|name| name.to_str().filter(|n| is_portable(n)))
        .collect::<Option<Vec<&str>>>()
        .ok_or(PickRejection::UnsupportedName)?;
    let rel = names.join("/");
    RelPath::new(rel.as_str()).map_err(|_| PickRejection::UnsupportedName)?;
    let mut ancestor = base.clone();
    for name in names.iter().take(names.len().saturating_sub(1)) {
        ancestor.push(name);
        if platform.metadata(&ancestor).map_err(rejection)?.kind == FileKind::Symlink {
            return Err(PickRejection::LinkedParent);
        }
    }
    Ok(Located {
        root: *root,
        rel,
        path,
        roots,
    })
}

/// A name Carrova can carry: no control or bidi characters, no `\`.
fn is_portable(name: &str) -> bool {
    !name.chars().any(|c| c == '\\' || is_unsafe_char(c))
}

/// Where a picked link may point: its own root or home, as capture allows.
fn link_scope(roots: &[(SemanticRoot, PathBuf)], root: SemanticRoot) -> Vec<&Path> {
    roots
        .iter()
        .filter(|(r, _)| *r == root || *r == SemanticRoot::Home)
        .map(|(_, dir)| dir.as_path())
        .collect()
}

/// Refuse the link at `path` unless its target lies inside `scope`.
fn check_link(platform: &dyn Platform, path: &Path, scope: &[&Path]) -> Result<(), PickRejection> {
    let target = platform.read_link(path).map_err(rejection)?;
    let resolved = path
        .parent()
        .and_then(|parent| normalize_abs(&parent.join(target)));
    match resolved {
        Some(t) if scope.iter().any(|dir| t.starts_with(dir)) => Ok(()),
        _ => Err(PickRejection::SymlinkOutside),
    }
}

/// Facts of the folder at `dir` (`rel` under its root; links never
/// followed), and whether it holds a secret location.
fn measure(dir: &Path, rel: &str) -> Result<(PickFacts, bool), PickRejection> {
    // shortcut: measuring is not cancellable; add a Cancel if users pick huge trees
    let (mut size, mut files, mut secret) = (0u64, 0u64, false);
    for entry in WalkDir::new(dir).follow_links(false) {
        let entry = match entry {
            Ok(e) => e,
            Err(e) if e.depth() == 0 => {
                return Err(PickRejection::Unreadable {
                    reason: e.to_string(),
                });
            }
            // Capture reports what it cannot read.
            Err(_) => continue,
        };
        // The whole path under the root: some secret locations span folders.
        if !secret && let Ok(below) = entry.path().strip_prefix(dir) {
            let below: Vec<_> = below.iter().map(|name| name.to_string_lossy()).collect();
            secret = secret_in(&format!("{rel}/{}", below.join("/"))).is_some();
        }
        if entry.file_type().is_file()
            && let Ok(m) = entry.metadata()
        {
            size += m.len();
            files += 1;
        }
    }
    Ok((facts(PickKind::Dir, size, files), secret))
}

const fn facts(kind: PickKind, size: u64, files: u64) -> PickFacts {
    PickFacts { kind, size, files }
}

/// A platform error on a picked path, in the user's words.
fn rejection(e: PlatformError) -> PickRejection {
    match e {
        PlatformError::Io { source, .. } if source.kind() == io::ErrorKind::NotFound => {
            PickRejection::NotFound
        }
        e => PickRejection::Unreadable {
            reason: e.to_string(),
        },
    }
}

impl Selection {
    /// Add `pick` as a raw item: folded into a picked folder that holds
    /// it, replacing picks it holds, or ignored as a duplicate of an
    /// earlier pick. A scanned raw row at the same path becomes the pick.
    /// A sensitive pick starts unticked unless already confirmed.
    pub fn add_pick(&mut self, pick: Pick) -> PickResult {
        let Pick {
            root,
            rel,
            facts,
            sensitive,
        } = pick;
        let path = RootedPath::new(root, &rel);
        let key = ItemKey::raw(path);
        if self.items.get(&key).is_some_and(|c| c.pick.is_some()) {
            return PickResult::Duplicate { item: key };
        }
        if let Some((into, _)) = self.picks().find(|(_, p)| path.is_within(*p)) {
            return PickResult::Absorbed { into: into.clone() };
        }
        let replaced: Vec<ItemKey> = self
            .picks()
            .filter(|(_, p)| p.is_within(path))
            .map(|(k, _)| k.clone())
            .collect();
        for k in &replaced {
            self.items.remove(k);
        }
        let choice = self.items.entry(key.clone()).or_default();
        choice.sensitive |= sensitive;
        choice.included = !choice.sensitive || choice.confirmed_sensitive;
        choice.paths = vec![path.to_string()];
        choice.pick = Some(facts);
        PickResult::Added {
            item: key,
            replaced,
        }
    }

    /// Every Manual pick, with its path.
    fn picks(&self) -> impl Iterator<Item = (&ItemKey, RootedPath<'_>)> {
        self.items
            .iter()
            .filter_map(|(k, c)| match (k.target(), &c.pick) {
                (Some(ItemTarget::Raw(p)), Some(_)) => Some((k, p)),
                _ => None,
            })
    }
}

/// A root in the user's words, as the desktop app names it
/// (`i18n/en/receive.ts` `mapping.roots`).
const fn root_words(root: SemanticRoot) -> &'static str {
    match root {
        SemanticRoot::UserConfig => "settings folder",
        SemanticRoot::UserData => "data folder",
        SemanticRoot::Cache => "cache folder",
        SemanticRoot::Home => "home folder",
        SemanticRoot::Documents => "Documents folder",
    }
}

#[cfg(test)]
#[path = "../../tests/select/pick/unit.rs"]
mod tests;
