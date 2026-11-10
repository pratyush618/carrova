use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::thread;

use carrova_bundle::{
    Asset, BundleError, FetchError, Fetcher, HttpConfig, HttpFetcher, UpdateOutcome, Updater,
};

use crate::support::{Keys, NOW, bundle, platform, store};

/// Serve files of `dir` over plain HTTP on localhost (one request per
/// connection); returns the base URL.
fn serve(dir: PathBuf) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            respond(stream, &dir);
        }
    });
    format!("http://{addr}/releases/download/recipes-v1/")
}

fn respond(mut stream: TcpStream, dir: &Path) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut request = String::new();
    reader.read_line(&mut request).unwrap();
    let mut line = String::new();
    while reader.read_line(&mut line).unwrap() > 2 {
        line.clear();
    }
    let path = request.split_whitespace().nth(1).unwrap_or("/");
    let name = path.rsplit('/').next().unwrap_or("");
    let (status, body) = match fs::read(dir.join(name)) {
        Ok(body) if !name.is_empty() => ("200 OK", body),
        _ => ("404 Not Found", Vec::new()),
    };
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(&body);
}

fn fetcher(base: String) -> HttpFetcher {
    HttpFetcher::new(&HttpConfig {
        require_https: false,
        ..HttpConfig::new(base)
    })
    .unwrap()
}

#[test]
fn http_fetcher_installs_a_bundle() {
    let tmp = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let out = tmp.path().join("out");
    bundle(&keys, tmp.path(), &out, 1, "Alpha");
    let base = serve(out);

    let p = platform(&tmp.path().join("home"));
    let store = store(&p, &keys.roots);
    let outcome = Updater::new(fetcher(base), keys.roots.clone())
        .update(&store, NOW)
        .unwrap();
    assert!(matches!(outcome, UpdateOutcome::Installed { .. }));
    assert_eq!(store.recipes().unwrap().recipes.len(), 2);
}

#[test]
fn http_fetcher_enforces_size_cap() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("srv");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join(Asset::SIGNATURE_FILE), vec![b' '; 64 * 1024]).unwrap();
    let f = fetcher(serve(dir));

    let err = f
        .get(&Asset::Signature, Asset::Signature.max_size())
        .unwrap_err();
    assert!(matches!(err, FetchError::TooLarge { .. }), "{err}");
    let err = f.get(&Asset::Index, 1024).unwrap_err();
    assert!(matches!(err, FetchError::NotFound), "{err}");

    let keys = Keys::new();
    let p = platform(&tmp.path().join("home"));
    let store = store(&p, &keys.roots);
    let err = Updater::new(f, keys.roots.clone())
        .check(&store, NOW)
        .unwrap_err();
    assert!(matches!(err, BundleError::Fetch { .. }), "{err}");
}

#[test]
fn plain_http_is_refused_by_default() {
    let err = HttpFetcher::new(&HttpConfig::new("http://example.invalid/x")).unwrap_err();
    assert!(matches!(err, FetchError::Config(_)), "{err}");
    HttpFetcher::new(&HttpConfig::new("https://example.invalid/x")).unwrap();
}
