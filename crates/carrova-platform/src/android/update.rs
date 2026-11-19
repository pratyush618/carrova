//! App update glue through `Updates.kt`: a package's facts (read by the
//! system's package manager), the install permission, the installer, and
//! the download's notification.

use std::path::Path;

use super::{events, files, jni};
use crate::{AppPackage, PlatformError, UpdateNotice, UpdateStage, UpdateTap};

const CONTEXT_TO_STRING: &str = "(Landroid/content/Context;)Ljava/lang/String;";
const PATH_TO_STRING: &str = "(Landroid/content/Context;Ljava/lang/String;)Ljava/lang/String;";
const CONTEXT_TO_VOID: &str = "(Landroid/content/Context;)V";

/// The facts of the APK at `apk`.
pub(super) fn inspect(apk: &Path) -> Result<AppPackage, PlatformError> {
    let path = utf8(apk)?;
    let json = jni::call("Updates.inspect", |env, context| {
        let path = env.new_string(path)?;
        let json = jni::call_static(
            env,
            context,
            "Updates",
            "inspect",
            PATH_TO_STRING,
            &[context.into(), (&path).into()],
        )?;
        files::string(env, json)
    })?;
    events::from_json("Updates.inspect", &json)
}

/// The facts of this app as installed.
pub(super) fn installed() -> Result<AppPackage, PlatformError> {
    let json = jni::call("Updates.installed", |env, context| {
        let json = jni::call_static(
            env,
            context,
            "Updates",
            "installed",
            CONTEXT_TO_STRING,
            &[context.into()],
        )?;
        files::string(env, json)
    })?;
    events::from_json("Updates.installed", &json)
}

/// `PackageManager.canRequestPackageInstalls()`.
pub(super) fn can_install() -> Result<bool, PlatformError> {
    jni::call("Updates.canInstall", |env, context| {
        jni::call_static(
            env,
            context,
            "Updates",
            "canInstall",
            "(Landroid/content/Context;)Z",
            &[context.into()],
        )?
        .z()
    })
}

/// Android's "Install unknown apps" page for this app.
pub(super) fn open_install_settings() -> Result<(), PlatformError> {
    context_only("Updates.openInstallSettings", "openInstallSettings")
}

/// Hand the APK at `apk` to Android's installer.
pub(super) fn install(apk: &Path) -> Result<(), PlatformError> {
    let path = utf8(apk)?;
    jni::call("Updates.install", |env, context| {
        let path = env.new_string(path)?;
        jni::call_static(
            env,
            context,
            "Updates",
            "install",
            "(Landroid/content/Context;Ljava/lang/String;)V",
            &[context.into(), (&path).into()],
        )?
        .v()
    })
}

/// Show `notice` as the download's notification, or remove it.
pub(super) fn show(notice: Option<&UpdateNotice>) -> Result<(), PlatformError> {
    let Some(notice) = notice else {
        return context_only("Updates.hide", "hide");
    };
    let stage = match notice.stage {
        UpdateStage::Downloading => 0,
        UpdateStage::Paused => 1,
        UpdateStage::WaitingForWifi => 2,
    };
    let long = |n: Option<u64>| n.map_or(-1, |n| i64::try_from(n).unwrap_or(i64::MAX));
    jni::call("Updates.show", |env, context| {
        let version = env.new_string(&notice.version)?;
        jni::call_static(
            env,
            context,
            "Updates",
            "show",
            "(Landroid/content/Context;Ljava/lang/String;IJJJ)V",
            &[
                context.into(),
                (&version).into(),
                stage.into(),
                long(Some(notice.done)).into(),
                long(notice.total).into(),
                long(notice.bytes_per_sec).into(),
            ],
        )?
        .v()
    })
}

/// The notification tap the user made last, taken once. Unreadable
/// counts as none: the download carries on as the app last said.
pub(super) fn take_tap() -> Option<UpdateTap> {
    let tap = jni::call("Updates.takeTap", |env, context| {
        let tap = jni::call_static(
            env,
            context,
            "Updates",
            "takeTap",
            "()Ljava/lang/String;",
            &[],
        )?
        .l()?;
        if tap.is_null() {
            return Ok(None);
        }
        Ok(Some(String::from(env.get_string(&tap.into())?)))
    });
    super::update_tap(tap.ok().flatten().as_deref())
}

/// `Updates.<method>(context)`.
fn context_only(call: &'static str, method: &str) -> Result<(), PlatformError> {
    jni::call(call, |env, context| {
        jni::call_static(
            env,
            context,
            "Updates",
            method,
            CONTEXT_TO_VOID,
            &[context.into()],
        )?
        .v()
    })
}

fn utf8(path: &Path) -> Result<&str, PlatformError> {
    path.to_str()
        .ok_or(PlatformError::InvalidInput("path is not UTF-8"))
}
