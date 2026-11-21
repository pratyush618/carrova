//! The feed, the APK's address, Wi-Fi and the release key.

use carrova_bundle::app_update::{
    ApkRejected, AppVersion, FEED_URL, RELEASE_CERT_SHA256, Release, check_apk, just_updated,
    may_download, newer, trusted_url,
};
use carrova_platform::AppPackage;

fn version(s: &str) -> AppVersion {
    s.parse().unwrap()
}

fn package(version_code: u64, signer: [u8; 32]) -> AppPackage {
    AppPackage {
        package: "org.byteveda.Carrova".to_owned(),
        version_code,
        signers: vec![signer],
    }
}

#[test]
fn versions_compare_by_number_not_text() {
    assert!(version("0.10.0") > version("0.9.9"));
    assert!(version("1.0.0") > version("0.99.99"));
    assert_eq!(version("v0.4.0"), version("0.4.0"));
    assert_eq!(version("0.4.0").to_string(), "0.4.0");
}

#[test]
fn anything_but_three_numbers_is_no_version() {
    for bad in [
        "",
        "0.4",
        "0.4.0.1",
        "0.4.x",
        "0.4.0-beta",
        "0..1",
        "+1.0.0",
    ] {
        assert!(bad.parse::<AppVersion>().is_err(), "{bad}");
    }
}

#[test]
fn the_feed_names_the_release_and_its_notes() {
    let feed = br#"{"version":"0.4.0","notes":"Faster transfers","pub_date":"2026-11-20T09:00:00Z",
        "platforms":{"linux-x86_64":{"signature":"s","url":"u"}}}"#;
    let release = Release::parse(feed).unwrap();
    assert_eq!(release.version, version("0.4.0"));
    assert_eq!(release.notes.as_deref(), Some("Faster transfers"));
    assert_eq!(release.date.as_deref(), Some("2026-11-20T09:00:00Z"));
}

#[test]
fn blank_notes_read_as_none_and_a_bad_version_is_refused() {
    let release = Release::parse(br#"{"version":"0.4.0","notes":"  "}"#).unwrap();
    assert_eq!(release.notes, None);
    assert!(Release::parse(br#"{"version":"latest"}"#).is_err());
    assert!(Release::parse(b"<html>").is_err());
}

#[test]
fn the_apk_is_the_release_asset_of_its_tag() {
    let release = Release::parse(br#"{"version":"0.4.0"}"#).unwrap();
    assert_eq!(
        release.apk_url(),
        "https://github.com/pratyush618/carrova/releases/download/v0.4.0/Carrova_0.4.0_android-arm64.apk"
    );
    assert!(trusted_url(&release.apk_url()));
}

#[test]
fn only_https_on_github_hosts_is_trusted() {
    assert!(trusted_url("https://objects.githubusercontent.com/a/b?x=1"));
    assert!(trusted_url(
        "https://release-assets.githubusercontent.com/a"
    ));
    assert!(trusted_url("https://GitHub.com/a"));
    for url in [
        "http://github.com/a",
        "https://github.com.evil.example/a",
        "https://evil.example/github.com",
        "https://user@github.com/a",
        "https://github.com:8443/a",
        "github.com/a",
    ] {
        assert!(!trusted_url(url), "{url}");
    }
}

#[test]
fn off_wifi_the_download_waits_unless_mobile_data_is_allowed() {
    assert!(!may_download(false, false));
    assert!(may_download(false, true));
    assert!(may_download(true, false));
}

#[test]
fn the_feed_is_the_desktop_updaters_on_the_same_repository() {
    assert_eq!(
        FEED_URL,
        "https://github.com/pratyush618/carrova/releases/latest/download/latest.json"
    );
    assert!(trusted_url(FEED_URL));
}

#[test]
fn a_newer_release_signed_apk_is_accepted() {
    let installed = package(3000, RELEASE_CERT_SHA256);
    assert_eq!(
        check_apk(&package(4000, RELEASE_CERT_SHA256), &installed),
        Ok(())
    );
}

#[test]
fn another_signer_is_rejected() {
    let installed = package(3000, RELEASE_CERT_SHA256);
    let apk = package(4000, [7; 32]);
    assert_eq!(check_apk(&apk, &installed), Err(ApkRejected::WrongSigner));
}

#[test]
fn an_extra_or_missing_signer_is_rejected() {
    let installed = package(3000, RELEASE_CERT_SHA256);
    let mut apk = package(4000, RELEASE_CERT_SHA256);
    apk.signers.push([7; 32]);
    assert_eq!(check_apk(&apk, &installed), Err(ApkRejected::WrongSigner));
    apk.signers.clear();
    assert_eq!(check_apk(&apk, &installed), Err(ApkRejected::WrongSigner));
}

#[test]
fn the_same_or_an_older_build_is_rejected() {
    let installed = package(3000, RELEASE_CERT_SHA256);
    for code in [3000, 2000] {
        let apk = package(code, RELEASE_CERT_SHA256);
        assert_eq!(check_apk(&apk, &installed), Err(ApkRejected::NotNewer));
    }
}

#[test]
fn another_app_is_rejected() {
    let installed = package(3000, RELEASE_CERT_SHA256);
    let apk = AppPackage {
        package: "com.example.other".to_owned(),
        ..package(4000, RELEASE_CERT_SHA256)
    };
    assert_eq!(check_apk(&apk, &installed), Err(ApkRejected::OtherApp));
}

#[test]
fn an_update_is_announced_once_and_never_on_a_first_run() {
    assert!(just_updated(Some("0.3.0"), "0.4.0"));
    assert!(!just_updated(Some("0.4.0"), "0.4.0"));
    assert!(!just_updated(None, "0.4.0"));
}

#[test]
fn only_a_newer_release_is_offered() {
    let release = Release::parse(br#"{"version":"0.4.0"}"#).unwrap();
    assert_eq!(newer(release.clone(), "0.3.9"), Ok(Some(release.clone())));
    assert_eq!(newer(release.clone(), "0.4.0"), Ok(None));
    assert_eq!(newer(release.clone(), "0.10.0"), Ok(None));
    assert!(newer(release, "dev").is_err());
}
