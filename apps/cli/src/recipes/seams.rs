//! Where the CLI gets its platform, bundle trust roots and HTTP settings,
//! plus the test seams of the bundle-update e2e (`tests/e2e/bundle/run.sh`)
//! and the cross-OS e2e (`tests/e2e/cross-os.sh`).
//!
//! The seams exist only in builds with the opt-in cargo feature
//! `e2e-seams` (never release packages, never the desktop app):
//!
//! * `CARROVA_BUNDLE_TEST_ROOTS=<roots.json>` trusts that roots file
//!   instead of the embedded release roots, for every bundle command
//!   (the installed bundle is re-verified on each load).
//! * `CARROVA_BUNDLE_ALLOW_HTTP=1` accepts plain `http://` (a loopback test
//!   server).
//! * `CARROVA_E2E_OS=windows|macos` runs on a fake platform with that OS's
//!   identity and root layout under `$HOME` (real files, Linux host).
//!
//! A build with the seams prints a loud warning whenever one is set.

use carrova_bundle::{HttpConfig, Result, TrustRoots, release_roots};
use carrova_platform::Platform;

/// The platform every command runs on: the current OS's adapter.
///
/// Fails only in `e2e-seams` builds, when a requested fake OS can't be
/// created (never falls back to the real machine).
pub fn platform() -> anyhow::Result<Box<dyn Platform>> {
    #[cfg(feature = "e2e-seams")]
    if let Some(fake) = enabled::fake_platform()? {
        return Ok(fake);
    }
    Ok(carrova_platform::current())
}

/// Roots every bundle command trusts.
pub fn trusted_roots() -> Result<TrustRoots> {
    #[cfg(feature = "e2e-seams")]
    if let Some(roots) = enabled::test_roots()? {
        return Ok(roots);
    }
    release_roots()
}

/// HTTP settings for the release base `url`.
#[must_use]
pub fn http_config(url: &str) -> HttpConfig {
    let config = HttpConfig::new(url);
    #[cfg(feature = "e2e-seams")]
    if enabled::allow_http() {
        return HttpConfig {
            require_https: false,
            ..config
        };
    }
    config
}

#[cfg(feature = "e2e-seams")]
mod enabled {
    use anyhow::Context as _;
    use carrova_bundle::{BundleError, Result, TrustRoots, parse_roots};
    use carrova_platform::fake::FakePlatform;
    use carrova_platform::{OsKind, Platform};

    /// Roots file trusted instead of the release roots.
    pub const TEST_ROOTS_ENV: &str = "CARROVA_BUNDLE_TEST_ROOTS";
    /// `1`: accept plain `http://` bundle URLs.
    pub const ALLOW_HTTP_ENV: &str = "CARROVA_BUNDLE_ALLOW_HTTP";
    /// `windows|macos|linux`: pretend to be that OS (fake layout under `$HOME`).
    pub const OS_ENV: &str = "CARROVA_E2E_OS";

    fn warn(what: &str) {
        eprintln!("WARNING: TEST BUILD (e2e-seams): {what}. Never use this build for real data.");
    }

    pub fn test_roots() -> Result<Option<TrustRoots>> {
        let Some(path) = std::env::var_os(TEST_ROOTS_ENV) else {
            return Ok(None);
        };
        warn(&format!(
            "{TEST_ROOTS_ENV} replaces the release trust roots"
        ));
        let text = std::fs::read_to_string(&path).map_err(|e| BundleError::Io {
            path: path.into(),
            source: e,
        })?;
        parse_roots(&text).map(Some)
    }

    pub fn allow_http() -> bool {
        let on = std::env::var_os(ALLOW_HTTP_ENV).is_some_and(|v| v == "1");
        if on {
            warn(&format!(
                "{ALLOW_HTTP_ENV} allows plain http:// bundle URLs"
            ));
        }
        on
    }

    /// The fake platform `CARROVA_E2E_OS` asks for, if set; an error when
    /// it can't be built, so a test run never touches the real home.
    pub fn fake_platform() -> anyhow::Result<Option<Box<dyn Platform>>> {
        let Some(value) = std::env::var_os(OS_ENV) else {
            return Ok(None);
        };
        let os = value
            .to_str()
            .with_context(|| format!("{OS_ENV} is not valid UTF-8"))?
            .parse::<OsKind>()
            .with_context(|| format!("{OS_ENV}={}", value.display()))?;
        let home = std::env::var_os("HOME")
            .with_context(|| format!("{OS_ENV} needs $HOME for the fake layout"))?;
        warn(&format!("{OS_ENV} fakes a {os} layout under $HOME"));
        Ok(Some(Box::new(FakePlatform::with_os_layout(home, os))))
    }
}
