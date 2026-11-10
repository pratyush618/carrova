//! Shape predicates over a whole token, named by the rows of
//! [`super::RULES`].

use std::net::{Ipv4Addr, Ipv6Addr, SocketAddrV4};

use super::Charset;

/// Shortest hex run treated as a hash or key (128 bits).
const MIN_HEX_LEN: usize = 32;
/// Shortest base64-like run treated as a secret.
const MIN_BASE64_LEN: usize = 40;
/// Hosts that identify public services, not the user; kept readable.
const PUBLIC_HOSTS: &[&str] = &[
    "github.com",
    "api.github.com",
    "objects.githubusercontent.com",
    "crates.io",
    "static.crates.io",
    "registry.npmjs.org",
];
/// Top-level domains that make a dotted word a hostname. Curated so file
/// names stay readable: no ccTLDs that double as extensions (`.rs`, `.md`,
/// `.sh`, `.py`, `.pl`, `.in`), plus the usual LAN pseudo-TLDs.
const HOST_TLDS: &[&str] = &[
    "com",
    "net",
    "org",
    "edu",
    "gov",
    "io",
    "dev",
    "app",
    "co",
    "ai",
    "cloud",
    "me",
    "info",
    "biz",
    "xyz",
    "tech",
    "uk",
    "de",
    "fr",
    "nl",
    "eu",
    "us",
    "ca",
    "au",
    "jp",
    "es",
    "it",
    "ch",
    "se",
    "local",
    "lan",
    "internal",
    "home",
    "corp",
    "localdomain",
];

/// `/anything` (Unix) or `X:\anything` (Windows).
pub(super) fn is_absolute_path(token: &str) -> bool {
    let bytes = token.as_bytes();
    let unix = bytes.len() > 1 && bytes[0] == b'/';
    let windows = bytes.len() > 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'\\' | b'/');
    unix || windows
}

/// `<1-3 digits>-<word>-<word>`, optionally a ticket `…@host:port`.
pub(super) fn is_pairing_code(token: &str) -> bool {
    let code = token.split_once('@').map_or(token, |(code, _)| code);
    let mut parts = code.split('-');
    let nameplate = parts.next().unwrap_or_default();
    let words: Vec<&str> = parts.collect();
    (1..=3).contains(&nameplate.len())
        && nameplate.bytes().all(|b| b.is_ascii_digit())
        && words.len() == 2
        && words
            .iter()
            .all(|word| !word.is_empty() && word.bytes().all(|b| b.is_ascii_alphabetic()))
}

/// `local@domain.tld` with an alphabetic top-level domain.
pub(super) fn is_email(token: &str) -> bool {
    let Some((local, domain)) = token.split_once('@') else {
        return false;
    };
    let local_ok = !local.is_empty()
        && local
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._%+-".contains(&b));
    let labels: Vec<&str> = domain.split('.').collect();
    let tld = labels.last().copied().unwrap_or_default();
    local_ok
        && labels.len() >= 2
        && labels.iter().all(|label| {
            !label.is_empty()
                && label
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        })
        && tld.len() >= 2
        && tld.bytes().all(|b| b.is_ascii_alphabetic())
}

/// Dotted IPv4, with or without `:port`.
pub(super) fn is_ipv4(token: &str) -> bool {
    token.parse::<Ipv4Addr>().is_ok() || token.parse::<SocketAddrV4>().is_ok()
}

/// Any IPv6 form `std` parses, except the bare `::` (no address digits).
pub(super) fn is_ipv6(token: &str) -> bool {
    token.bytes().any(|b| b.is_ascii_hexdigit()) && token.parse::<Ipv6Addr>().is_ok()
}

/// `header.payload.signature`, base64url parts, header starting `eyJ` (`{"`).
pub(super) fn is_jwt(token: &str) -> bool {
    let parts: Vec<&str> = token.split('.').collect();
    token.starts_with("eyJ")
        && parts.len() == 3
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.bytes().all(|b| Charset::WordDash.admits(b)))
}

/// `label.label…tld` with a known TLD, unless it is a [`PUBLIC_HOSTS`] entry.
pub(super) fn is_hostname(token: &str) -> bool {
    let labels: Vec<&str> = token.split('.').collect();
    let tld = labels.last().copied().unwrap_or_default();
    labels.len() >= 2
        && HOST_TLDS
            .iter()
            .any(|known| known.eq_ignore_ascii_case(tld))
        && labels.iter().all(|label| is_dns_label(label))
        && !PUBLIC_HOSTS
            .iter()
            .any(|host| host.eq_ignore_ascii_case(token))
}

fn is_dns_label(label: &str) -> bool {
    (1..=63).contains(&label.len())
        && !label.starts_with('-')
        && !label.ends_with('-')
        && label.bytes().all(|b| Charset::AlnumDash.admits(b))
}

/// Hashes, key fingerprints, raw keys.
pub(super) fn is_long_hex(token: &str) -> bool {
    token.len() >= MIN_HEX_LEN && token.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Long mixed-case alphanumeric runs (standard or URL-safe base64): API
/// keys, encoded secrets. Requiring upper, lower and digit keeps long
/// identifiers and words readable.
pub(super) fn is_long_base64(token: &str) -> bool {
    let bytes = token.as_bytes();
    bytes.len() >= MIN_BASE64_LEN
        && bytes
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || b"+/=_-".contains(b))
        && bytes.iter().any(u8::is_ascii_uppercase)
        && bytes.iter().any(u8::is_ascii_lowercase)
        && bytes.iter().any(u8::is_ascii_digit)
}
