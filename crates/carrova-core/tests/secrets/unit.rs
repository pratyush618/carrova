use super::*;

#[test]
fn secret_locations_are_named_in_user_words() {
    assert_eq!(secret_in(".ssh/config"), Some("SSH keys"));
    assert_eq!(
        secret_in("work/.aws/credentials"),
        Some("cloud credentials")
    );
    assert_eq!(secret_in("site/.env"), Some("environment secrets"));
    assert_eq!(secret_in("tls/server.pem"), Some("a key or certificate"));
}

#[test]
fn private_keys_are_secret_but_public_halves_are_not() {
    assert!(secret_in("proj/id_ed25519").is_some());
    assert!(secret_in("proj/id_ed25519.pub").is_none());
}

#[test]
fn ordinary_files_are_not_secret() {
    assert!(secret_in("notes/keystore.txt").is_none());
    assert!(secret_in("notes/env").is_none());
}

#[test]
fn token_files_are_secret_under_any_root() {
    let cases = [
        // (under home, under its XDG root)
        (".git-credentials", None),
        (".docker/config.json", None),
        (".npmrc", None),
        (".pypirc", None),
        (".config/gh/hosts.yml", Some("gh/hosts.yml")),
        (".config/rclone/rclone.conf", Some("rclone/rclone.conf")),
        (".password-store/mail.gpg", None),
        (
            ".local/share/keyrings/login.keyring",
            Some("keyrings/login.keyring"),
        ),
        (".mozilla/firefox/p.default/key4.db", None),
        (".mozilla/firefox/p.default/logins.json", None),
        (
            ".config/chromium/Default/Login Data",
            Some("chromium/Default/Login Data"),
        ),
        (
            ".config/gcloud/credentials.db",
            Some("gcloud/credentials.db"),
        ),
        (".azure/msal_token_cache.json", None),
        (".terraform.d/credentials.tfrc.json", None),
    ];
    for (home, xdg) in cases {
        assert!(secret_in(home).is_some(), "{home}");
        if let Some(xdg) = xdg {
            assert!(secret_in(xdg).is_some(), "{xdg}");
        }
    }
}

#[test]
fn a_multi_component_rule_needs_its_components_in_a_row() {
    assert!(secret_in("gh/hosts.yml").is_some());
    assert!(secret_in("gh/notes/hosts.yml").is_none());
    assert!(secret_in("docker/config.json").is_none());
    assert!(secret_in("app/config.json").is_none());
}

#[test]
fn matching_ignores_case() {
    assert_eq!(secret_in(".SSH/authorized_keys"), Some("SSH keys"));
    assert!(secret_in("keys/ID_RSA").is_some());
    assert!(secret_in("keys/ID_RSA.PUB").is_none());
    assert!(secret_in("tls/SERVER.PEM").is_some());
    assert!(secret_in(".Config/GH/Hosts.yml").is_some());
}

#[test]
fn env_file_variants_are_secret() {
    for rel in ["app/.env.production", "app/.env.local", "app/.ENV.Local"] {
        assert_eq!(secret_in(rel), Some("environment secrets"), "{rel}");
    }
    assert!(secret_in("app/env.local").is_none());
}
