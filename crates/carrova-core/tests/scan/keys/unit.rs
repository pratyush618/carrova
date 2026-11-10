use super::*;
use crate::scan::tests::{ED25519_FP, ED25519_PUB};

/// `ssh-keygen -t ecdsa -b 256 -C ''` (trailing space, no comment).
const ECDSA_PUB: &str = "ecdsa-sha2-nistp256 AAAAE2VjZHNhLXNoYTItbmlzdHAyNTYAAAAIbmlzdHAyNTYAAABBBPy+/cnQwjfBFUSC6Rs7WrMZZAyseRScrGd2mCjW9aS6pCtyceJHT8nMPK7K0N2Ghd8PN9RnHvLj/MFRamcY/l4= ";
const ECDSA_FP: &str = "SHA256:qJp++Qyt8v2HmKN2FMVB739SLaAQsRksjf8mT5sEksk";

#[test]
fn a_public_key_line_gives_the_ssh_keygen_fingerprint_kind_and_comment() {
    let key = PublicKey::parse(ED25519_PUB).unwrap();
    assert_eq!(key.kind, KeyKind::Ed25519);
    assert_eq!(key.fingerprint, ED25519_FP);
    assert_eq!(key.comment.as_deref(), Some("ana@laptop"));

    let key = PublicKey::parse(ECDSA_PUB).unwrap();
    assert_eq!(key.kind, KeyKind::Ecdsa);
    assert_eq!(key.fingerprint, ECDSA_FP);
    assert_eq!(key.comment, None);
}

#[test]
fn the_kind_comes_from_the_type_string() {
    assert_eq!(KeyKind::of("ssh-rsa"), KeyKind::Rsa);
    assert_eq!(KeyKind::of("ecdsa-sha2-nistp521"), KeyKind::Ecdsa);
    assert_eq!(KeyKind::of("sk-ssh-ed25519@openssh.com"), KeyKind::Other);
}

#[test]
fn unparseable_lines_are_not_keys() {
    for line in ["", "ssh-ed25519", "ssh-ed25519 !!!notbase64 x", "garbage\n"] {
        assert_eq!(PublicKey::parse(line), None, "{line:?}");
    }
}
