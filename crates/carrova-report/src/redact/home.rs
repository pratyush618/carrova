//! Collapsing paths below the home directory: `~/clients/acme/.env` →
//! `~/<path>/.env`. Only the final file extension survives, because
//! directory and file names below `~` routinely name clients, projects
//! and people.

/// Placeholder for the home directory.
pub(crate) const HOME: &str = "~";
/// Longest extension kept; longer "extensions" are names in disguise.
const MAX_EXTENSION_LEN: usize = 10;

/// `tail` is what follows the home directory (empty or starting with a
/// separator). `<path>` stands for one or more path components; the file
/// type is kept as `*.ext` (or `.name` for a dotfile such as `.env`).
pub(crate) fn collapse(tail: &str) -> String {
    let separators = ['/', '\\'];
    if tail.trim_matches(separators).is_empty() {
        return format!("{HOME}{tail}");
    }
    let file = tail.rsplit(separators).next().unwrap_or_default();
    match file_type(file) {
        Some(kind) => format!("{HOME}/<path>/{kind}"),
        None => format!("{HOME}/<path>"),
    }
}

/// `.env` for a dotfile, `*.yml` for `hosts.yml`, `None` without a
/// short alphanumeric extension.
fn file_type(file: &str) -> Option<String> {
    let (stem, extension) = file.rsplit_once('.')?;
    let short_alphanumeric = (1..=MAX_EXTENSION_LEN).contains(&extension.len())
        && extension.bytes().all(|b| b.is_ascii_alphanumeric());
    match stem {
        _ if !short_alphanumeric => None,
        "" => Some(format!(".{extension}")),
        _ => Some(format!("*.{extension}")),
    }
}
