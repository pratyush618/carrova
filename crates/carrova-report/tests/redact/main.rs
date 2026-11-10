//! Redaction policy through the public API: home and system paths, user
//! names, URL credentials, secret assignments, vendor tokens, hostnames,
//! idempotency and the published rule list.

use std::path::Path;

use carrova_report::Redactor;

fn redactor() -> Redactor {
    Redactor::new(Some(Path::new("/home/ana")))
}

#[test]
fn tilde_paths_already_in_the_text_collapse_too() {
    assert_eq!(
        Redactor::default().redact("cannot open ~/clients/acme/secret-plans.carrova"),
        "cannot open ~/<path>/*.carrova"
    );
}

#[test]
fn long_or_unusual_extensions_are_dropped_with_the_name() {
    assert_eq!(
        redactor().redact("/home/ana/x/report.acme-q3-final and /home/ana/.ssh-agent-sock"),
        "~/<path> and ~/<path>"
    );
}

#[test]
fn url_credentials_are_replaced_and_hosts_redacted() {
    assert_eq!(
        Redactor::default().redact("fetch https://user:pass@host/repo.git failed"),
        "fetch https://<cred>@host/repo.git failed"
    );
    assert_eq!(
        Redactor::default().redact("clone https://ana:s3cr3t@git.acme.io:8443/x."),
        "clone https://<cred>@<host>:8443/x."
    );
    assert_eq!(
        Redactor::default().redact("GET https://github.com/acme/carrova/releases"),
        "GET https://github.com/acme/carrova/releases"
    );
}

#[test]
fn url_query_secrets_are_replaced_and_other_values_kept() {
    assert_eq!(
        Redactor::default().redact("GET https://github.com/x?page=2&access_token=abc123#top"),
        "GET https://github.com/x?page=2&access_token=<secret>#top"
    );
}

#[test]
fn file_urls_follow_the_path_policy() {
    assert_eq!(
        redactor().redact("file:///home/ana/acme/notes.txt and file:///srv/acme"),
        "file://~/<path>/*.txt and file://<path>"
    );
}

#[test]
fn values_assigned_to_secret_keys_are_replaced() {
    let redactor = Redactor::default();
    assert_eq!(redactor.redact("password=hunter2"), "password=<secret>");
    assert_eq!(redactor.redact("token: abc, next"), "token: <secret>, next");
    assert_eq!(
        redactor.redact(r#"{"password": "hunter2"}"#),
        r#"{"password": "<secret>"}"#
    );
    assert_eq!(
        redactor.redact("GITHUB_TOKEN = 'xyz'"),
        "GITHUB_TOKEN = '<secret>'"
    );
    assert_eq!(
        redactor.redact("--password=hunter2."),
        "--password=<secret>."
    );
}

#[test]
fn secret_key_words_without_an_assignment_are_left_alone() {
    let text = "token expired; password reset needed";
    assert_eq!(Redactor::default().redact(text), text);
}

#[test]
fn bearer_credentials_are_replaced() {
    assert_eq!(
        Redactor::default().redact("Authorization: Bearer abc.def-ghi rejected"),
        "Authorization: Bearer <secret> rejected"
    );
}

#[test]
fn aws_secret_keys_are_replaced_only_when_they_look_like_one() {
    let redactor = Redactor::default();
    assert_eq!(
        redactor.redact("aws_secret_access_key = wjalrxutnfemik/k7mdengbpxrficyexamplekey"),
        "aws_secret_access_key = <secret>"
    );
    assert_eq!(
        redactor.redact("aws_secret_access_key = missing"),
        "aws_secret_access_key = missing"
    );
}

#[test]
fn realistic_error_chain_with_planted_secrets_comes_out_clean() {
    let chain = [
        "LAN session 7-orbit-lemon@192.168.1.44:47821 failed",
        "applying item ssh/keys for ana (ana.lopez@corp.example.com)",
        "renaming \"/home/ana/.local/state/carrova/staging/files/12\" to \"/home/ana/clients/acme/.env\"",
        "chunk 9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08 mismatch",
        "token npm_A1b2C3d4E5f6G7h8I9j0 rejected by registry.npmjs.org via fe80::2",
        "git fetch https://ana:Tr0ub4dor@git.acme-internal.com/acme/infra.git: 403",
        "hook failed: STRIPE_KEY=sk_live_51HxYzAbCdEfGhIjKlMnOp gitlab glpat-xYz12-AbCdEfGhIjKlMn",
        "slack xoxb-1234567890-0987654321-AbCdEfGhIjKl notify failed",
        "aws: AKIAIOSFODNN7EXAMPLE aws_secret_access_key=wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY",
        "curl -H 'Authorization: Bearer opaque-session-value' https://api.acme.io/v1?api_key=k-998877",
        "db connect password=hunter2 host=db01.prod.acme.io",
        "Permission denied (os error 13)",
    ];
    let redacted: Vec<String> = chain.iter().map(|line| redactor().redact(line)).collect();
    assert_eq!(
        redacted,
        [
            "LAN session <pairing-code> failed",
            "applying item ssh/keys for <user> (<email>)",
            "renaming \"~/<path>\" to \"~/<path>/.env\"",
            "chunk <hex> mismatch",
            "token <token> rejected by registry.npmjs.org via <ip>",
            "git fetch https://<cred>@<host>/acme/infra.git: 403",
            "hook failed: STRIPE_KEY=<token> gitlab <token>",
            "slack <token> notify failed",
            "aws: <token> aws_secret_access_key=<secret>",
            "curl -H 'Authorization: Bearer <secret>' https://<host>/v1?api_key=<secret>",
            "db connect password=<secret> host=<host>",
            "Permission denied (os error 13)",
        ]
    );
    let planted = [
        "ana",
        "192.168",
        "orbit",
        "9f86d0",
        "npm_",
        "fe80",
        "/home",
        "clients",
        "Tr0ub4dor",
        "acme-internal",
        "sk_live",
        "glpat",
        "xoxb",
        "AKIA",
        "wJalr",
        "opaque",
        "k-998877",
        "hunter2",
        "db01",
        "api.acme",
        "corp.example",
    ];
    for line in &redacted {
        for secret in planted {
            assert!(!line.contains(secret), "{secret:?} leaked in {line:?}");
        }
    }
}

#[test]
fn home_paths_collapse_to_tilde_path_keeping_only_the_final_extension() {
    assert_eq!(
        redactor().redact("cannot read /home/ana/.config/gh/hosts.yml: denied"),
        "cannot read ~/<path>/*.yml: denied"
    );
    assert_eq!(
        redactor().redact("loading /home/ana/clients/acme/.env"),
        "loading ~/<path>/.env"
    );
    assert_eq!(
        redactor().redact("key /home/ana/.ssh/id_ed25519 is open"),
        "key ~/<path> is open"
    );
    assert_eq!(redactor().redact("home is /home/ana."), "home is ~.");
}

#[test]
fn other_users_homes_and_system_paths_become_path_placeholders() {
    assert_eq!(
        redactor().redact("open \"/home/anabel/notes\" and /etc/hosts."),
        "open \"<path>\" and <path>."
    );
}

#[test]
fn user_name_is_redacted_as_a_whole_word_only() {
    assert_eq!(
        redactor().redact("owner ana, group ana_dev, host banana"),
        "owner <user>, group ana_dev, host banana"
    );
    assert_eq!(redactor().redact("ana@laptop"), "<user>@laptop");
}

#[test]
fn sensitive_segments_below_home_disappear_in_the_collapse() {
    assert_eq!(
        redactor().redact("/home/ana/mail/bob@example.com/ghp_abcdefABCDEF0123456789"),
        "~/<path>"
    );
}

#[test]
fn trailing_punctuation_survives_redaction() {
    assert_eq!(
        Redactor::default().redact("peer 10.0.0.2:47821, code 7-orbit-lemon."),
        "peer <ip>, code <pairing-code>."
    );
}

#[test]
fn colon_separated_parts_are_checked_individually() {
    let hash = "0f".repeat(32);
    assert_eq!(
        Redactor::default().redact(&format!("chunk blake3:{hash} at addr:192.168.0.9")),
        "chunk blake3:<hex> at addr:<ip>"
    );
}

#[test]
fn ipv6_in_brackets_is_redacted() {
    assert_eq!(
        Redactor::default().redact("connect [fe80::1ff:fe23:4567:890a]:443 failed"),
        "connect [<ip>]:443 failed"
    );
}

#[test]
fn ordinary_error_text_and_rust_paths_are_left_alone() {
    let text = "carrova_core::engine: plan rejected (3 conflicts); retry with --force";
    assert_eq!(redactor().redact(text), text);
    let text = "Vec<String> in recipe.yaml line 3";
    assert_eq!(redactor().redact(text), text);
}

#[test]
fn redaction_is_idempotent() {
    let once = redactor().redact(
        "ghp_abcdefABCDEF0123456789 at /home/ana/x/a.yml and /srv/y, \
         https://u:p@git.acme.io/r?token=t password=p ~/.env",
    );
    assert_eq!(redactor().redact(&once), once);
}

#[test]
fn unknown_home_skips_only_the_contextual_rules() {
    assert_eq!(
        Redactor::new(None).redact("/home/ana/x by ana"),
        "<path> by ana"
    );
}

#[test]
fn realistic_error_chain_comes_out_clean() {
    let chain = [
        "LAN session 7-orbit-lemon@192.168.1.44:47821 failed",
        "applying item ssh/keys for ana (ana.lopez@corp.example.com)",
        "renaming \"/home/ana/.local/state/carrova/staging/files/12\" to \"/home/ana/.ssh/id_ed25519\"",
        "chunk 9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08 mismatch",
        "token npm_A1b2C3d4E5f6G7h8I9j0 rejected by registry.npmjs.org via fe80::2",
        "Permission denied (os error 13)",
    ];
    let redacted: Vec<String> = chain.iter().map(|line| redactor().redact(line)).collect();
    assert_eq!(
        redacted,
        [
            "LAN session <pairing-code> failed",
            "applying item ssh/keys for <user> (<email>)",
            "renaming \"~/<path>\" to \"~/<path>\"",
            "chunk <hex> mismatch",
            "token <token> rejected by registry.npmjs.org via <ip>",
            "Permission denied (os error 13)",
        ]
    );
    for line in &redacted {
        for secret in ["ana", "192.168", "orbit", "9f86d0", "npm_", "fe80", "/home"] {
            assert!(!line.contains(secret), "{secret:?} leaked in {line:?}");
        }
    }
}

#[test]
fn policy_lists_every_rule_in_match_order() {
    let names: Vec<&str> = Redactor::policy().map(|(name, _)| name).collect();
    assert_eq!(names.first(), Some(&"bearer credential"));
    for name in [
        "URL credentials",
        "bearer credential",
        "absolute path",
        "hostname",
        "user name",
    ] {
        assert!(names.contains(&name), "{name} missing");
    }
}
