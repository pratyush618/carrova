//! The token policy table: one row per kind of sensitive token.

use super::matchers::{
    is_absolute_path, is_device_fingerprint, is_email, is_hostname, is_ipv4, is_ipv6, is_jwt,
    is_long_base64, is_long_hex, is_pairing_code,
};
use super::{Charset, Matcher, Rule};

/// Body length of tokens with an open-ended length.
const TOKEN_BODY: (usize, usize) = (16, usize::MAX);

/// The token policy, first match wins. Contextual rules (home directory,
/// URLs, secret assignments, user name) run around this table in
/// [`crate::Redactor`]. Order matters where shapes overlap: emails and
/// IPs before hostnames, vendor tokens before the generic hex/base64 runs.
pub(crate) const RULES: &[Rule] = &[
    Rule {
        name: "absolute path",
        matcher: Matcher::Shape(is_absolute_path),
        replacement: "<path>",
    },
    Rule {
        name: "pairing code",
        matcher: Matcher::Shape(is_pairing_code),
        replacement: "<pairing-code>",
    },
    Rule {
        name: "device fingerprint",
        matcher: Matcher::Shape(is_device_fingerprint),
        replacement: "<device>",
    },
    Rule {
        name: "email address",
        matcher: Matcher::Shape(is_email),
        replacement: "<email>",
    },
    Rule {
        name: "IPv4 address",
        matcher: Matcher::Shape(is_ipv4),
        replacement: "<ip>",
    },
    Rule {
        name: "IPv6 address",
        matcher: Matcher::Shape(is_ipv6),
        replacement: "<ip>",
    },
    Rule {
        name: "GitHub token",
        // Personal, OAuth, user-to-server, server, refresh, fine-grained.
        matcher: Matcher::Prefixed {
            prefixes: &["ghp_", "gho_", "ghu_", "ghs_", "ghr_", "github_pat_"],
            body_len: TOKEN_BODY,
            charset: Charset::Word,
        },
        replacement: "<token>",
    },
    Rule {
        name: "npm token",
        matcher: Matcher::Prefixed {
            prefixes: &["npm_"],
            body_len: TOKEN_BODY,
            charset: Charset::Word,
        },
        replacement: "<token>",
    },
    Rule {
        name: "Stripe key",
        matcher: Matcher::Prefixed {
            prefixes: &["sk_live_", "sk_test_", "rk_live_", "rk_test_"],
            body_len: TOKEN_BODY,
            charset: Charset::Word,
        },
        replacement: "<token>",
    },
    Rule {
        name: "GitLab token",
        matcher: Matcher::Prefixed {
            prefixes: &["glpat-"],
            body_len: (20, usize::MAX),
            charset: Charset::WordDash,
        },
        replacement: "<token>",
    },
    Rule {
        name: "Slack token",
        matcher: Matcher::Prefixed {
            prefixes: &["xoxa-", "xoxb-", "xoxp-", "xoxr-", "xoxs-"],
            body_len: (10, usize::MAX),
            charset: Charset::AlnumDash,
        },
        replacement: "<token>",
    },
    Rule {
        name: "AWS access key ID",
        // Long-term (AKIA) and temporary (ASIA) key IDs.
        matcher: Matcher::Prefixed {
            prefixes: &["AKIA", "ASIA"],
            body_len: (16, 16),
            charset: Charset::UpperAlnum,
        },
        replacement: "<token>",
    },
    Rule {
        name: "JSON web token",
        matcher: Matcher::Shape(is_jwt),
        replacement: "<token>",
    },
    Rule {
        name: "hostname",
        matcher: Matcher::Shape(is_hostname),
        replacement: "<host>",
    },
    Rule {
        name: "long hex string",
        matcher: Matcher::Shape(is_long_hex),
        replacement: "<hex>",
    },
    Rule {
        name: "long base64 string",
        matcher: Matcher::Shape(is_long_base64),
        replacement: "<secret>",
    },
];
