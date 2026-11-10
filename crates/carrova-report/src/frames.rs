//! Backtrace filtering: keep only symbol names of Carrova's own crates.

/// Symbol prefix shared by every Carrova crate (`carrova_core::…`, `carrova_desktop::…`).
const OUR_CRATE_PREFIX: &str = "carrova_";
/// Rust legacy-mangling hash suffix (`::h0123456789abcdef`).
const HASH_SUFFIX_LEN: usize = 19;

/// Extracts the symbol names of Carrova frames from a rendered
/// [`std::backtrace::Backtrace`]. File paths (`at …` lines), addresses and
/// third-party/std frames are dropped; consecutive duplicates collapse.
#[must_use]
pub(crate) fn our_frames(rendered: &str) -> Vec<String> {
    let mut frames: Vec<String> = Vec::new();
    for symbol in rendered.lines().filter_map(frame_symbol) {
        let symbol = strip_hash(symbol);
        let is_ours = symbol.trim_start_matches('<').starts_with(OUR_CRATE_PREFIX);
        if is_ours && frames.last().is_none_or(|last| last != symbol) {
            frames.push(symbol.to_owned());
        }
    }
    frames
}

/// `"  12: carrova_core::x::y"` → `"carrova_core::x::y"`; `None` for other lines.
fn frame_symbol(line: &str) -> Option<&str> {
    let (index, symbol) = line.trim_start().split_once(": ")?;
    index
        .bytes()
        .all(|b| b.is_ascii_digit())
        .then(|| symbol.trim())
}

fn strip_hash(symbol: &str) -> &str {
    let Some(cut) = symbol.len().checked_sub(HASH_SUFFIX_LEN) else {
        return symbol;
    };
    match symbol.get(cut..).and_then(|tail| tail.strip_prefix("::h")) {
        Some(hash) if hash.bytes().all(|b| b.is_ascii_hexdigit()) => &symbol[..cut],
        _ => symbol,
    }
}

#[cfg(test)]
#[path = "../tests/frames/unit.rs"]
mod tests;
