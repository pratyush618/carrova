use carrova_platform::SemanticRoot;

use super::*;
use crate::space::FsLocation;

fn every() -> Vec<Blocker> {
    vec![
        Blocker::UnconfirmedSensitive {
            item: "ssh/keys".into(),
            reason: "marked sensitive in the archive; controls SSH (home:.ssh)".into(),
        },
        Blocker::RunningApp {
            app: "app".into(),
            name: "The App".into(),
        },
        Blocker::FreeSpace(SpaceShortfall {
            needed: 67_108_899,
            available: 1000,
            location: FsLocation {
                state_dir: true,
                roots: vec![SemanticRoot::UserConfig, SemanticRoot::Home],
            },
        }),
        Blocker::FreeSpace(SpaceShortfall {
            needed: 1,
            available: 0,
            location: FsLocation {
                state_dir: false,
                roots: vec![SemanticRoot::Documents],
            },
        }),
        Blocker::FreeSpace(SpaceShortfall {
            needed: 1,
            available: 0,
            location: FsLocation {
                state_dir: true,
                roots: Vec::new(),
            },
        }),
    ]
}

#[test]
fn display_texts_are_the_persisted_format() {
    let texts: Vec<String> = every().iter().map(ToString::to_string).collect();
    assert_eq!(
        texts,
        [
            "`ssh/keys` is sensitive (marked sensitive in the archive; controls SSH (home:.ssh)): confirm it explicitly to import it, or deselect it",
            "The App (`app`) is running: close it before applying, or override the requires-closed check",
            "not enough free space: 67108899 bytes needed on the filesystem of the state dir and user_config, home (incl. margin), 1000 available",
            "not enough free space: 1 bytes needed on the filesystem of documents (incl. margin), 0 available",
            "not enough free space: 1 bytes needed on the filesystem of the state dir (incl. margin), 0 available",
        ]
    );
}

#[test]
fn blockers_serialise_as_tagged_objects() {
    let json = serde_json::to_value(every()).unwrap();
    assert_eq!(
        json[0],
        serde_json::json!({
            "kind": "unconfirmed_sensitive",
            "item": "ssh/keys",
            "reason": "marked sensitive in the archive; controls SSH (home:.ssh)",
        })
    );
    assert_eq!(
        json[1],
        serde_json::json!({ "kind": "running_app", "app": "app", "name": "The App" })
    );
    assert_eq!(
        json[2],
        serde_json::json!({
            "kind": "free_space",
            "needed": 67_108_899,
            "available": 1000,
            "location": { "state_dir": true, "roots": ["user_config", "home"] },
        })
    );
    assert_eq!(
        serde_json::from_value::<Vec<Blocker>>(json).unwrap(),
        every()
    );
}

/// [`read_blockers`] over a JSON array.
fn read(json: &str) -> Result<Vec<Blocker>, serde_json::Error> {
    read_blockers(&mut serde_json::Deserializer::from_str(json))
}

#[test]
fn stored_texts_of_older_plans_still_read() {
    for b in every() {
        let text = serde_json::to_string(&[b.to_string()]).unwrap();
        assert_eq!(read(&text).unwrap(), std::slice::from_ref(&b));
        let tagged = serde_json::to_string(&[&b]).unwrap();
        assert_eq!(read(&tagged).unwrap(), [b]);
    }
}

#[test]
fn unknown_text_is_rejected() {
    for s in [
        "",
        "app running",
        "`x` is running",
        "not enough free space: lots",
    ] {
        assert!(legacy::parse(s).is_none(), "{s}");
        assert!(read(&serde_json::to_string(&[s]).unwrap()).is_err(), "{s}");
    }
}

#[test]
fn running_app_texts_of_older_plans_read_with_the_id_as_name() {
    let old = "`app` is running: close it before applying, or override the requires-closed check";
    assert_eq!(
        legacy::parse(old).unwrap(),
        Blocker::RunningApp {
            app: "app".into(),
            name: "app".into()
        }
    );
}

#[test]
fn a_stored_name_with_parentheses_still_reads_back() {
    let blocker = Blocker::RunningApp {
        app: "x".into(),
        name: "X (beta) (`odd`".into(),
    };
    assert_eq!(legacy::parse(&blocker.to_string()).unwrap(), blocker);
}
