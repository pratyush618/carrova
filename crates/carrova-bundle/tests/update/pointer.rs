//! The release pointer: `recipes-latest/latest.json` names the immutable
//! `recipes-vN/` release that holds the bundle.

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Component, Path, PathBuf};
use std::thread;

use carrova_bundle::{
    BundleError, BundleVersion, FetchError, Fetcher, HttpConfig, HttpFetcher, LATEST_POINTER,
    UpdateOutcome, Updater,
};

use crate::support::{Keys, NOW, bundle, platform, store};

const PREFIX: &str = "/releases/download/";

/// Serve `root` (one request per connection) under `PREFIX`; returns the
/// base URL clients are configured with.
fn serve(root: PathBuf) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            respond(stream, &root);
        }
    });
    format!("http://{addr}{PREFIX}")
}

fn respond(mut stream: TcpStream, root: &Path) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut request = String::new();
    reader.read_line(&mut request).unwrap();
    let mut line = String::new();
    while reader.read_line(&mut line).unwrap() > 2 {
        line.clear();
    }
    let path = request.split_whitespace().nth(1).unwrap_or("/");
    let rel = Path::new(path.strip_prefix(PREFIX).unwrap_or(""));
    let safe = rel.components().all(|c| matches!(c, Component::Normal(_)));
    let (status, body) = match fs::read(root.join(rel)) {
        Ok(body) if safe && !rel.as_os_str().is_empty() => ("200 OK", body),
        _ => ("404 Not Found", Vec::new()),
    };
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(&body);
}

fn config(base: String) -> HttpConfig {
    HttpConfig {
        require_https: false,
        ..HttpConfig::new(base)
    }
}

fn write_pointer(root: &Path, body: &str) {
    let path = root.join(LATEST_POINTER);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, body).unwrap();
}

#[test]
fn the_pointer_selects_the_immutable_release_to_install() {
    let tmp = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let root = tmp.path().join("srv");
    bundle(&keys, tmp.path(), &root.join("recipes-v1"), 1, "One");
    bundle(&keys, tmp.path(), &root.join("recipes-v2"), 2, "Two");
    write_pointer(&root, r#"{ "version": 2 }"#);
    let fetcher = HttpFetcher::latest(&config(serve(root))).unwrap();
    assert_eq!(fetcher.release(), Some(BundleVersion(2)));

    let p = platform(&tmp.path().join("home"));
    let store = store(&p, &keys.roots);
    let outcome = Updater::new(fetcher, keys.roots.clone())
        .update(&store, NOW)
        .unwrap();
    assert!(
        matches!(outcome, UpdateOutcome::Installed { .. }),
        "{outcome:?}"
    );
    assert_eq!(
        store.installed().unwrap().unwrap().version(),
        BundleVersion(2)
    );
}

#[test]
fn a_pointer_disagreeing_with_the_release_index_is_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let root = tmp.path().join("srv");
    bundle(&keys, tmp.path(), &root.join("recipes-v3"), 2, "Two");
    write_pointer(&root, r#"{"version":3}"#);
    let fetcher = HttpFetcher::latest(&config(serve(root))).unwrap();
    let p = platform(&tmp.path().join("home"));
    let err = Updater::new(fetcher, keys.roots.clone())
        .check(&store(&p, &keys.roots), NOW)
        .unwrap_err();
    assert!(
        matches!(
            err,
            BundleError::PointerMismatch {
                pointer: BundleVersion(3),
                index: BundleVersion(2)
            }
        ),
        "{err}"
    );
}

#[test]
fn malformed_pointers_are_refused_before_any_release_url_is_built() {
    for body in [
        r#"{"version":0}"#,
        r#"{"version":"../../x"}"#,
        r#"{"version":-1}"#,
        r#"{"version":1,"extra":true}"#,
        "recipes-v1",
    ] {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("srv");
        write_pointer(&root, body);
        let err = HttpFetcher::latest(&config(serve(root))).unwrap_err();
        assert!(
            matches!(err, BundleError::InvalidPointer(_)),
            "{body}: {err}"
        );
    }
}

#[test]
fn an_oversized_pointer_is_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("srv");
    write_pointer(&root, &format!("{}{{\"version\":1}}", " ".repeat(4096)));
    let err = HttpFetcher::latest(&config(serve(root))).unwrap_err();
    assert!(
        matches!(
            err,
            BundleError::Fetch {
                source: FetchError::TooLarge { .. },
                ..
            }
        ),
        "{err}"
    );
}

#[test]
fn a_missing_pointer_is_not_found_so_clients_retry_later() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("srv");
    fs::create_dir_all(&root).unwrap();
    let err = HttpFetcher::latest(&config(serve(root))).unwrap_err();
    assert!(
        matches!(
            err,
            BundleError::Fetch {
                source: FetchError::NotFound,
                ..
            }
        ),
        "{err}"
    );
}
