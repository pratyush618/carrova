//! Unit tests for the redaction token rules (wired from
//! `src/redact/rules/mod.rs`; private `RULES` and `classify`).

use super::*;

fn rule_name(token: &str) -> Option<&'static str> {
    classify(token).map(|rule| rule.name)
}

#[test]
fn every_rule_has_a_placeholder_in_angle_brackets() {
    for rule in RULES {
        assert!(rule.replacement.starts_with('<') && rule.replacement.ends_with('>'));
    }
}

#[test]
fn absolute_paths_match_on_unix_and_windows_but_not_relative() {
    assert_eq!(rule_name("/etc/ssh/sshd_config"), Some("absolute path"));
    assert_eq!(rule_name(r"C:\Users\ana\key"), Some("absolute path"));
    assert_eq!(rule_name("src/main.rs"), None);
    assert_eq!(rule_name("/"), None);
}

#[test]
fn pairing_codes_and_tickets_match() {
    assert_eq!(rule_name("7-orbit-lemon"), Some("pairing code"));
    assert_eq!(rule_name("123-Orbit-Lemon"), Some("pairing code"));
    assert_eq!(
        rule_name("7-orbit-lemon@192.168.1.4:47000"),
        Some("pairing code")
    );
    assert_eq!(rule_name("1234-orbit-lemon"), None);
    assert_eq!(rule_name("7-orbit"), None);
}

#[test]
fn emails_need_an_alphabetic_tld() {
    assert_eq!(
        rule_name("ana.lopez+dev@example.co.uk"),
        Some("email address")
    );
    assert_eq!(rule_name("user@10.0.0.1"), None);
    assert_eq!(rule_name("@example.com"), None);
}

#[test]
fn ip_addresses_match_with_and_without_port() {
    assert_eq!(rule_name("192.168.1.20"), Some("IPv4 address"));
    assert_eq!(rule_name("10.0.0.7:47821"), Some("IPv4 address"));
    assert_eq!(rule_name("fe80::1ff:fe23:4567:890a"), Some("IPv6 address"));
    assert_eq!(rule_name("::1"), Some("IPv6 address"));
    assert_eq!(rule_name("::"), None);
    assert_eq!(rule_name("carrova_core::engine"), None);
    assert_eq!(rule_name("1.2.3"), None);
}

#[test]
fn known_token_prefixes_match() {
    assert_eq!(rule_name("ghp_16CharsAtLeast1234"), Some("GitHub token"));
    assert_eq!(
        rule_name("github_pat_11ABCDEFG0123456789_abc"),
        Some("GitHub token")
    );
    assert_eq!(rule_name("npm_abcdefABCDEF012345"), Some("npm token"));
    assert_eq!(rule_name("ghp_short"), None);
}

#[test]
fn long_hex_matches_from_32_digits() {
    assert_eq!(rule_name(&"a1".repeat(16)), Some("long hex string"));
    assert_eq!(rule_name(&"a1".repeat(15)), None);
}

#[test]
fn long_base64_needs_mixed_case_and_digits() {
    assert_eq!(
        rule_name("QWxhZGRpbjpvcGVuIHNlc2FtZQ9xYz8kLmNvbS9hYmM="),
        Some("long base64 string")
    );
    assert_eq!(
        rule_name("a_very_long_snake_case_identifier_without_digits"),
        None
    );
}

#[test]
fn stripe_keys_match_live_test_and_restricted() {
    assert_eq!(
        rule_name("sk_live_4eC39HqLyjWDarjtT1zdp7dc"),
        Some("Stripe key")
    );
    assert_eq!(
        rule_name("sk_test_4eC39HqLyjWDarjtT1zdp7dc"),
        Some("Stripe key")
    );
    assert_eq!(
        rule_name("rk_live_4eC39HqLyjWDarjtT1zdp7dc"),
        Some("Stripe key")
    );
    assert_eq!(rule_name("sk_live_short"), None);
}

#[test]
fn gitlab_tokens_match() {
    assert_eq!(
        rule_name("glpat-xYz12-AbCdEfGhIjKlMn"),
        Some("GitLab token")
    );
    assert_eq!(rule_name("glpat-short"), None);
}

#[test]
fn slack_tokens_match_every_kind() {
    for kind in ["a", "b", "p", "r", "s"] {
        let token = format!("xox{kind}-1234567890-abcdefABCDEF");
        assert_eq!(rule_name(&token), Some("Slack token"), "{token}");
    }
    assert_eq!(rule_name("xoxz-1234567890-abcdef"), None);
}

#[test]
fn aws_access_key_ids_need_sixteen_upper_alphanumerics() {
    assert_eq!(rule_name("AKIAIOSFODNN7EXAMPLE"), Some("AWS access key ID"));
    assert_eq!(rule_name("AKIAIOSFODNN7EXAMPL"), None);
    assert_eq!(rule_name("AKIAiosfodnn7example"), None);
}

#[test]
fn json_web_tokens_match() {
    assert_eq!(
        rule_name(
            "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0In0.dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PlFUP0THsR8U"
        ),
        Some("JSON web token")
    );
}

#[test]
fn hostnames_match_beyond_the_allow_list_but_file_names_do_not() {
    assert_eq!(rule_name("build.corp.example.com"), Some("hostname"));
    assert_eq!(rule_name("acme.io"), Some("hostname"));
    assert_eq!(rule_name("ana-laptop.local"), Some("hostname"));
    assert_eq!(rule_name("github.com"), None);
    assert_eq!(rule_name("GitHub.com"), None);
    assert_eq!(rule_name("registry.npmjs.org"), None);
    for file in [
        "hosts.yml",
        "Cargo.toml",
        "main.rs",
        "notes.md",
        "archive.tar.gz",
        "e.g",
    ] {
        assert_eq!(rule_name(file), None, "{file}");
    }
}

#[test]
fn device_fingerprints_match_only_in_their_shown_form() {
    assert_eq!(rule_name("7F3A\u{b7}19C2"), Some("device fingerprint"));
    assert_eq!(rule_name("7f3a\u{b7}19c2"), Some("device fingerprint"));
    assert_eq!(rule_name("7F3A\u{b7}19C"), None);
    assert_eq!(rule_name("7F3G\u{b7}19C2"), None);
    assert_eq!(rule_name("7F3A19C2"), None);
}
