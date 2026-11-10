use super::*;

#[test]
fn key_file_round_trips_and_checks_role() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("keys/root.key");
    let key = SigningKey::generate().unwrap();
    write(&path, Role::Root, &key).unwrap();
    assert_eq!(
        read(&path, Role::Root).unwrap().verifying_key(),
        key.verifying_key()
    );
    assert!(read(&path, Role::Signer).is_err());
    assert!(write(&path, Role::Root, &key).is_err(), "never overwrites");
}

#[test]
fn key_file_is_private() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("signer.key");
    write(&path, Role::Signer, &SigningKey::generate().unwrap()).unwrap();
    let meta = carrova_platform::current().metadata(&path).unwrap();
    assert_eq!(meta.mode, Some(0o600));
}
