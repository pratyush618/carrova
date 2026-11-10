//! The rewrite itself: plain safe Rust, testable on the host.

/// One OS family's path spelling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Os {
    /// Path separator (`/` or `\`).
    pub sep: u8,
}

impl Os {
    /// `windows` → `\`, anything else → `/` (the context's OS ids).
    #[must_use]
    pub fn from_id(id: &str) -> Self {
        Self {
            sep: if id == "windows" { b'\\' } else { b'/' },
        }
    }
}

/// What to rewrite: `from` home on `from_os` → `to` home on `to_os`.
#[derive(Debug, Clone, Copy)]
pub struct Homes<'a> {
    /// Home folder on the old computer.
    pub from: &'a str,
    /// Home folder on this computer.
    pub to: &'a str,
    /// The old computer's OS.
    pub from_os: Os,
    /// This computer's OS.
    pub to_os: Os,
}

/// The value of `key` in the context (`key=value` lines); empty if absent.
#[must_use]
pub fn context_value<'a>(ctx: &'a str, key: &str) -> &'a str {
    ctx.lines()
        .filter_map(|line| line.split_once('='))
        .find(|(k, _)| *k == key)
        .map_or("", |(_, v)| v)
}

/// Rewrite every path starting with the old home in `input`. JSON files
/// (`json`) hold paths as escaped strings: backslashes are read and
/// written doubled, and a path runs to the closing quote. Elsewhere a path
/// ends at whitespace or a quote. Separators after the home are converted
/// to this computer's. A home that is a prefix of a longer name
/// (`/home/a` in `/home/ab`) is left alone.
#[must_use]
pub fn rewrite(input: &[u8], homes: &Homes<'_>, json: bool) -> Vec<u8> {
    if homes.from.is_empty() || homes.to.is_empty() {
        return input.to_vec();
    }
    let from = spell(homes.from.as_bytes(), json);
    let to = spell(homes.to.as_bytes(), json);
    let from_sep = spell(&[homes.from_os.sep], json);
    let to_sep = spell(&[homes.to_os.sep], json);
    let ends_path = |b: u8| b == b'"' || (!json && (b == b'\'' || b.is_ascii_whitespace()));

    let mut out = Vec::with_capacity(input.len());
    let mut i = 0;
    while i < input.len() {
        let rest = &input[i..];
        let after = rest.get(from.len()..).unwrap_or_default();
        let at_boundary = after.is_empty() || after.starts_with(&from_sep) || ends_path(after[0]);
        if !(rest.starts_with(&from) && at_boundary) {
            out.push(input[i]);
            i += 1;
            continue;
        }
        out.extend_from_slice(&to);
        i += from.len();
        while i < input.len() && !ends_path(input[i]) {
            if input[i..].starts_with(&from_sep) {
                out.extend_from_slice(&to_sep);
                i += from_sep.len();
            } else {
                out.push(input[i]);
                i += 1;
            }
        }
    }
    out
}

/// `raw` as it appears in the file: JSON doubles backslashes.
fn spell(raw: &[u8], json: bool) -> Vec<u8> {
    if !json {
        return raw.to_vec();
    }
    raw.iter()
        .flat_map(|&b| {
            if b == b'\\' {
                vec![b'\\', b'\\']
            } else {
                vec![b]
            }
        })
        .collect()
}
