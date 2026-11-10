//! URLs, redacted by structure: `scheme://<cred>@host:port/path?query`.
//!
//! The user-info part is always replaced (`<cred>`); host, path segments
//! and query values go through the token policy; values of secret query
//! keys (`access_token=…`) are replaced outright; `file:` URLs follow
//! the path policy.

use super::{Redactor, keyed, lex};

/// Placeholder for the user-info part of a URL.
pub(crate) const CREDENTIALS: &str = "<cred>";

impl Redactor {
    /// Redacts `token` if it is a URL.
    pub(super) fn redact_url(&self, token: &str) -> Option<String> {
        let scheme_len = lex::scheme_len(token)?;
        let (scheme, rest) = token.split_at(scheme_len);
        let location_end = rest.find(['?', '#']).unwrap_or(rest.len());
        let (location, rest) = rest.split_at(location_end);
        let (query, fragment) = rest.split_once('#').unwrap_or((rest, ""));

        let mut out = String::from(scheme);
        if scheme.eq_ignore_ascii_case("file://") {
            // `file://` + absolute path (or an already collapsed `~/…`).
            out.push_str(&self.redact_word(location));
        } else {
            let (authority, path) = location.split_at(location.find('/').unwrap_or(location.len()));
            let host = match authority.rsplit_once('@') {
                Some((_, host)) => {
                    out.push_str(CREDENTIALS);
                    out.push('@');
                    host
                }
                None => authority,
            };
            out.push_str(&self.redact_parts(host));
            let segments: Vec<String> = path.split('/').map(|s| self.redact_parts(s)).collect();
            out.push_str(&segments.join("/"));
        }
        if let Some(query) = query.strip_prefix('?') {
            out.push('?');
            out.push_str(&self.redact_query(query));
        }
        if !fragment.is_empty() || rest.ends_with('#') {
            out.push('#');
            out.push_str(&self.redact_parts(fragment));
        }
        Some(out)
    }

    fn redact_query(&self, query: &str) -> String {
        let pairs: Vec<String> = query
            .split('&')
            .map(|pair| match pair.split_once('=') {
                Some((key, value)) => match keyed::assigned_secret(key, value) {
                    Some(rule) => format!("{key}={}", rule.replacement),
                    None => format!("{}={}", self.redact_parts(key), self.redact_parts(value)),
                },
                None => self.redact_parts(pair),
            })
            .collect();
        pairs.join("&")
    }
}
