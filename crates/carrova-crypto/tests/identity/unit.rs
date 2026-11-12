use data_encoding::HEXLOWER;
use sha2::{Digest, Sha256};

use super::*;
use crate::pairing::{ChannelBinding, EXPORTER_LEN, PakeRole};
use crate::sign::{KeyRole, SignContext};

fn key(seed: u8) -> DeviceKey {
    DeviceKey::from_bytes(&[seed; DEVICE_SEED_LEN])
}

fn binding(exporter: u8) -> ChannelBinding {
    ChannelBinding::new([exporter; EXPORTER_LEN], b"sender cert", b"receiver cert")
}

#[test]
fn signature_verifies_for_the_signers_role_and_binding() {
    let (me, peer) = (key(1), key(2));
    let sig = peer.sign_binding(PakeRole::Sender, &binding(7));
    assert_eq!(
        me.verify_peer(&peer.public(), PakeRole::Sender, &binding(7), &sig),
        Ok(())
    );
}

#[test]
fn signature_over_other_binding_fails() {
    let (me, peer) = (key(1), key(2));
    let sig = peer.sign_binding(PakeRole::Sender, &binding(7));
    assert_eq!(
        me.verify_peer(&peer.public(), PakeRole::Sender, &binding(8), &sig),
        Err(IdentityError::BadSignature)
    );
}

#[test]
fn signature_for_the_other_role_fails() {
    let (me, peer) = (key(1), key(2));
    let sig = peer.sign_binding(PakeRole::Sender, &binding(7));
    assert_eq!(
        me.verify_peer(&peer.public(), PakeRole::Receiver, &binding(7), &sig),
        Err(IdentityError::BadSignature)
    );
}

#[test]
fn signature_by_another_key_fails() {
    let (me, peer, claimed) = (key(1), key(2), key(3));
    let sig = peer.sign_binding(PakeRole::Sender, &binding(7));
    assert_eq!(
        me.verify_peer(&claimed.public(), PakeRole::Sender, &binding(7), &sig),
        Err(IdentityError::BadSignature)
    );
}

#[test]
fn peer_claiming_our_own_key_is_rejected() {
    let me = key(1);
    let sig = me.sign_binding(PakeRole::Sender, &binding(7));
    assert_eq!(
        me.verify_peer(&me.public(), PakeRole::Sender, &binding(7), &sig),
        Err(IdentityError::OwnKey)
    );
}

#[test]
fn signed_bytes_are_tag_role_binding_and_own_key() {
    let me = key(1);
    let b = binding(7);
    let mut payload = PakeRole::Receiver.identity().to_vec();
    payload.extend_from_slice(b.as_bytes());
    payload.extend_from_slice(me.public().as_bytes());
    let sig = crate::sign::Signature::from_bytes(&me.sign_binding(PakeRole::Receiver, &b));
    assert_eq!(
        me.0.verifying_key()
            .verify(SignContext::DeviceIdentity, &payload, &sig),
        Ok(())
    );
    assert_eq!(
        SignContext::DeviceIdentity.tag(),
        b"carrova/device-identity/v1\0"
    );
}

#[test]
fn bundle_signer_may_not_sign_device_identity() {
    assert!(!KeyRole::BundleSigner.permits(SignContext::DeviceIdentity));
}

#[test]
fn fingerprint_display_format() {
    let public = key(1).public();
    let digest = Sha256::digest(public.as_bytes());
    let hex = data_encoding::HEXUPPER.encode(&digest[..4]);
    let shown = Fingerprint::of(&public).to_string();
    assert_eq!(shown, format!("{}\u{b7}{}", &hex[..4], &hex[4..]));
}

#[test]
fn fingerprint_parses_with_or_without_separator_in_any_case() {
    let fp = Fingerprint::of(&key(1).public());
    let plain = fp.to_string().replace('\u{b7}', "");
    for typed in [
        fp.to_string(),
        plain.clone(),
        plain.to_lowercase(),
        plain.replacen(&plain[4..], &format!("-{}", &plain[4..]), 1),
    ] {
        assert_eq!(typed.parse::<Fingerprint>(), Ok(fp), "{typed}");
    }
    assert!("7F3A·19C".parse::<Fingerprint>().is_err());
    assert!("7F3A·19CG".parse::<Fingerprint>().is_err());
}

#[test]
fn public_key_round_trips_as_hex_and_serde() {
    let public = key(1).public();
    let hex = public.to_hex();
    assert_eq!(hex, HEXLOWER.encode(public.as_bytes()));
    assert_eq!(hex.parse::<DevicePublic>(), Ok(public));
    let json = serde_json::to_string(&public).unwrap();
    assert_eq!(json, format!("\"{hex}\""));
    assert_eq!(serde_json::from_str::<DevicePublic>(&json).unwrap(), public);
}

#[test]
fn seed_round_trips_and_generated_keys_differ() {
    let k = DeviceKey::generate().unwrap();
    assert_eq!(DeviceKey::from_bytes(k.expose_seed()).public(), k.public());
    assert_ne!(DeviceKey::generate().unwrap().public(), k.public());
}

#[test]
fn debug_output_has_no_key_bytes() {
    let k = key(1);
    let public = k.public();
    let seed_hex = HEXLOWER.encode(k.expose_seed());
    let public_hex = public.to_hex();
    let shown = format!("{k:?} {public:?}");
    assert!(
        !shown.contains(&seed_hex) && !shown.contains(&public_hex),
        "{shown}"
    );
    assert!(!shown.contains(&public_hex[..16]), "{shown}");
    assert!(
        shown.contains(&Fingerprint::of(&public).to_string()),
        "{shown}"
    );
}
