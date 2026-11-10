//! The rewrite on the host (the module is the same code, built for wasm).

use path_rewrite::rewrite::{Homes, Os, context_value, rewrite};

const LINUX: Os = Os { sep: b'/' };
const WINDOWS: Os = Os { sep: b'\\' };

fn homes<'a>(from: &'a str, to: &'a str, from_os: Os, to_os: Os) -> Homes<'a> {
    Homes {
        from,
        to,
        from_os,
        to_os,
    }
}

fn run(input: &str, h: &Homes<'_>, json: bool) -> String {
    String::from_utf8(rewrite(input.as_bytes(), h, json)).unwrap()
}

#[test]
fn linux_to_macos_json() {
    let h = homes("/home/ada", "/Users/ada", LINUX, LINUX);
    let input = r#"{"vaults":{"a":{"path":"/home/ada/Notes/Work","open":true},"b":{"path":"/home/adam/x"}}}"#;
    assert_eq!(
        run(input, &h, true),
        r#"{"vaults":{"a":{"path":"/Users/ada/Notes/Work","open":true},"b":{"path":"/home/adam/x"}}}"#
    );
}

#[test]
fn linux_to_windows_json_escapes_backslashes() {
    let h = homes("/home/ada", r"C:\Users\ada", LINUX, WINDOWS);
    let input = r#"{"path":"/home/ada/My Notes/sub"}"#;
    assert_eq!(
        run(input, &h, true),
        r#"{"path":"C:\\Users\\ada\\My Notes\\sub"}"#
    );
}

#[test]
fn windows_to_linux_json() {
    let h = homes(r"C:\Users\ada", "/home/ada", WINDOWS, LINUX);
    let input = r#"{"path":"C:\\Users\\ada\\Notes\\a.md"}"#;
    assert_eq!(run(input, &h, true), r#"{"path":"/home/ada/Notes/a.md"}"#);
}

#[test]
fn plain_text_paths_end_at_whitespace() {
    let h = homes("/home/ada", r"C:\Users\ada", LINUX, WINDOWS);
    let input = "dir = /home/ada/src/x other/path\nhome=/home/ada\n";
    assert_eq!(
        run(input, &h, false),
        "dir = C:\\Users\\ada\\src\\x other/path\nhome=C:\\Users\\ada\n"
    );
}

#[test]
fn an_empty_home_changes_nothing() {
    let h = homes("", "/Users/ada", LINUX, LINUX);
    assert_eq!(run("/home/ada/x", &h, false), "/home/ada/x");
}

#[test]
fn context_values_are_read_by_key() {
    let ctx = "source_home=/home/ada\ntarget_os=macos\n";
    assert_eq!(context_value(ctx, "source_home"), "/home/ada");
    assert_eq!(context_value(ctx, "target_os"), "macos");
    assert_eq!(context_value(ctx, "target_home"), "");
}
