use std::path::{Path, PathBuf};

use carrova_platform::fake::FakePlatform;
use tempfile::TempDir;

use super::*;
use crate::select::{ItemChoice, SelectionEdit};

/// A fake home `h` (roots Linux-style under it) in a temp dir.
struct Home {
    _tmp: TempDir,
    h: PathBuf,
    p: FakePlatform,
}

impl Home {
    fn new() -> Self {
        let tmp = TempDir::new().unwrap();
        let h = tmp.path().join("h");
        std::fs::create_dir_all(&h).unwrap();
        let p = FakePlatform::with_home(&h);
        Self { _tmp: tmp, h, p }
    }

    fn put(&self, rel: &str, bytes: &[u8]) {
        let path = self.h.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }

    fn at(&self, rel: &str) -> PathBuf {
        self.h.join(rel)
    }

    fn locate(&self, rel: &str) -> Result<(SemanticRoot, String), PickRejection> {
        locate_pick(&self.p, &self.at(rel))
    }

    fn resolve(&self, rel: &str) -> Result<Pick, PickRejection> {
        resolve_pick(&self.p, &self.at(rel))
    }
}

fn pick(rel: &str, sensitive: bool) -> Pick {
    Pick {
        root: SemanticRoot::Home,
        rel: rel.into(),
        facts: PickFacts {
            kind: PickKind::Dir,
            size: 0,
            files: 0,
        },
        sensitive,
    }
}

fn raw(rel: &str) -> ItemKey {
    ItemKey::raw(RootedPath::new(SemanticRoot::Home, rel))
}

#[test]
fn pick_uses_most_specific_root() {
    let home = Home::new();
    home.put(".config/foo/a", b"x");

    let located = home.locate(".config/foo");

    assert_eq!(located, Ok((SemanticRoot::UserConfig, "foo".into())));
}

#[test]
fn pick_of_a_root_is_refused() {
    let home = Home::new();

    assert_eq!(
        home.locate(""),
        Err(PickRejection::IsRoot {
            root: SemanticRoot::Home
        })
    );
    assert_eq!(
        home.locate(".config"),
        Err(PickRejection::IsRoot {
            root: SemanticRoot::UserConfig
        })
    );
}

#[test]
fn pick_holding_a_root_is_refused() {
    let home = Home::new();
    home.put(".local/share/x", b"x");

    assert_eq!(
        home.locate(".local"),
        Err(PickRejection::ContainsRoot {
            root: SemanticRoot::UserData
        })
    );
}

#[test]
fn pick_outside_roots_is_refused() {
    let home = Home::new();

    assert_eq!(
        locate_pick(&home.p, Path::new("/etc/hosts")),
        Err(PickRejection::OutsideRoots)
    );
    assert_eq!(
        locate_pick(&home.p, Path::new("relative/x")),
        Err(PickRejection::OutsideRoots)
    );
    assert_eq!(
        locate_pick(&home.p, &home.at("../elsewhere")),
        Err(PickRejection::OutsideRoots)
    );
}

#[test]
#[cfg(unix)] // Windows splits names at `\` and refuses control characters itself
fn pick_with_control_char_name_is_refused() {
    let home = Home::new();

    assert_eq!(home.locate("a\u{7}b"), Err(PickRejection::UnsupportedName));
    assert_eq!(home.locate("a\\b"), Err(PickRejection::UnsupportedName));
}

#[test]
fn pick_of_missing_path_is_not_found() {
    let home = Home::new();

    assert_eq!(home.resolve("gone"), Err(PickRejection::NotFound));
}

#[test]
fn deep_folder_pick_counts_files_and_bytes() {
    let home = Home::new();
    home.put("proj/a.txt", b"hello");
    home.put("proj/deep/er/b.txt", b"world!");

    let pick = home.resolve("proj").unwrap();

    assert_eq!(
        pick.facts,
        PickFacts {
            kind: PickKind::Dir,
            size: 11,
            files: 2
        }
    );
    assert_eq!((pick.root, pick.rel.as_str()), (SemanticRoot::Home, "proj"));
    assert!(!pick.sensitive);
}

#[test]
fn single_file_pick_is_measured() {
    let home = Home::new();
    home.put("notes/today.txt", b"abc");

    let pick = home.resolve("notes/today.txt").unwrap();

    assert_eq!(
        pick.facts,
        PickFacts {
            kind: PickKind::File,
            size: 3,
            files: 1
        }
    );
}

#[test]
fn pick_containing_ssh_key_is_sensitive() {
    let home = Home::new();
    home.put("proj/keys/id_rsa", b"secret");
    home.put("site/.env", b"TOKEN=1");

    assert!(home.resolve("proj").unwrap().sensitive);
    assert!(home.resolve("site/.env").unwrap().sensitive);
}

#[test]
fn folder_holding_a_multi_part_secret_location_is_sensitive() {
    let home = Home::new();
    home.put(".config/gh/hosts.yml", b"token");
    home.put("work/gh/notes.txt", b"x");

    assert!(home.resolve(".config/gh").unwrap().sensitive);
    assert!(home.resolve(".config/gh/hosts.yml").unwrap().sensitive);
    assert!(!home.resolve("work").unwrap().sensitive);
}

#[cfg(unix)] // symlinks need privileges on Windows
mod links {
    use carrova_platform::fake::symlink;

    use super::*;

    #[test]
    fn pick_under_linked_parent_is_refused() {
        let home = Home::new();
        home.put("real/proj/a", b"x");
        symlink(home.at("real"), home.at("link")).unwrap();

        assert_eq!(home.locate("link/proj"), Err(PickRejection::LinkedParent));
    }

    #[test]
    fn pick_symlink_to_outside_is_refused() {
        let home = Home::new();
        symlink("/etc/hosts", home.at("hosts")).unwrap();

        assert_eq!(home.resolve("hosts"), Err(PickRejection::SymlinkOutside));
    }

    #[test]
    fn pick_symlink_inside_roots_moves_as_link() {
        let home = Home::new();
        home.put("notes.txt", b"abc");
        symlink("notes.txt", home.at("current")).unwrap();

        let pick = home.resolve("current").unwrap();

        assert_eq!(pick.facts.kind, PickKind::Symlink);
    }

    #[test]
    fn pick_of_fifo_is_special() {
        let home = Home::new();
        let status = std::process::Command::new("mkfifo")
            .arg(home.at("pipe"))
            .status()
            .unwrap();
        assert!(status.success());

        assert_eq!(home.resolve("pipe"), Err(PickRejection::Special));
    }
}

#[test]
fn add_pick_absorbs_child() {
    let mut sel = Selection::default();
    sel.add_pick(pick("p", false));

    let result = sel.add_pick(pick("p/a", false));

    assert_eq!(result, PickResult::Absorbed { into: raw("p") });
    assert_eq!(sel.items.len(), 1);
}

#[test]
fn add_pick_parent_replaces_children() {
    let mut sel = Selection::default();
    sel.add_pick(pick("p/a", false));
    sel.add_pick(pick("p/b", false));
    sel.add_pick(pick("pq", false));

    let result = sel.add_pick(pick("p", false));

    assert_eq!(
        result,
        PickResult::Added {
            item: raw("p"),
            replaced: vec![raw("p/a"), raw("p/b")]
        }
    );
    let keys: Vec<_> = sel.items.keys().cloned().collect();
    assert_eq!(keys, [raw("p"), raw("pq")]);
}

#[test]
fn add_pick_twice_is_duplicate() {
    let mut sel = Selection::default();
    sel.add_pick(pick("p", false));

    let result = sel.add_pick(pick("p", false));

    assert_eq!(result, PickResult::Duplicate { item: raw("p") });
}

#[test]
fn picking_a_scanned_raw_row_promotes_it_to_a_pick() {
    let mut sel = Selection::default();
    sel.items.insert(raw("p"), ItemChoice::default());
    sel.items.insert(raw("keys"), ItemChoice::default());

    let p = sel.add_pick(pick("p", false));
    let keys = sel.add_pick(pick("keys", true));

    let added = |item| PickResult::Added {
        item,
        replaced: Vec::new(),
    };
    assert_eq!((p, keys), (added(raw("p")), added(raw("keys"))));
    let (p, keys) = (&sel.items[&raw("p")], &sel.items[&raw("keys")]);
    assert_eq!(p.pick, Some(pick("p", false).facts));
    assert!(p.included && !p.sensitive);
    assert!(keys.pick.is_some() && keys.sensitive && !keys.included);
    assert_eq!(
        sel.add_pick(pick("p", false)),
        PickResult::Duplicate { item: raw("p") }
    );
}

#[test]
fn picking_a_confirmed_scanned_raw_row_keeps_it_ticked() {
    let mut sel = Selection::default();
    let confirmed = ItemChoice {
        included: true,
        sensitive: true,
        confirmed_sensitive: true,
        ..ItemChoice::default()
    };
    sel.items.insert(raw("keys"), confirmed);

    sel.add_pick(pick("keys", true));

    let keys = &sel.items[&raw("keys")];
    assert!(keys.pick.is_some() && keys.included && keys.confirmed_sensitive);
}

#[test]
fn sensitive_pick_starts_unticked() {
    let mut sel = Selection::default();
    sel.add_pick(pick("keys", true));
    sel.add_pick(pick("notes", false));

    let keys = &sel.items[&raw("keys")];
    let notes = &sel.items[&raw("notes")];

    assert!(keys.sensitive && !keys.included);
    assert!(!notes.sensitive && notes.included);
    assert_eq!(notes.paths, ["home:notes"]);
    assert_eq!(notes.pick, Some(pick("notes", false).facts));
}

#[test]
fn remove_pick_drops_only_picks() {
    let mut sel = Selection::default();
    sel.add_pick(pick("p", false));
    sel.items.insert("app/config".into(), ItemChoice::default());

    let recipe = sel.apply(SelectionEdit::RemovePick {
        item: "app/config".into(),
    });
    let picked = sel.apply(SelectionEdit::RemovePick { item: raw("p") });

    assert!(recipe.is_err());
    assert_eq!(picked, Ok(()));
    let keys: Vec<_> = sel.items.keys().cloned().collect();
    assert_eq!(keys, [ItemKey::from("app/config")]);
}

#[test]
fn root_refusals_name_the_folder_in_plain_words() {
    let held = PickRejection::ContainsRoot {
        root: SemanticRoot::UserData,
    };
    assert_eq!(
        held.to_string(),
        "it holds your data folder; pick folders inside it"
    );
    let whole = PickRejection::IsRoot {
        root: SemanticRoot::Home,
    };
    assert_eq!(
        whole.to_string(),
        "it is your whole home folder; pick folders inside it"
    );
}
