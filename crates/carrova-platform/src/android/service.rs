//! The link service through `Service.kt`: Android's foreground service that
//! keeps the process (and the link supervisor) running, its notification,
//! and the pause the user sets there.

use ::jni::objects::JObject;

use super::jni;
use crate::{LinkNotice, PlatformError};

/// Run the service showing `notice` (started, or its notification
/// updated), or stop it.
pub(super) fn keep_running(notice: Option<&LinkNotice>) -> Result<(), PlatformError> {
    let Some(notice) = notice else {
        return jni::call("Service.stop", |env, context| {
            jni::call_static(
                env,
                context,
                "Service",
                "stop",
                "(Landroid/content/Context;)V",
                &[context.into()],
            )?
            .v()
        });
    };
    jni::call("Service.keepRunning", |env, context| {
        let len = i32::try_from(notice.devices.len()).unwrap_or(i32::MAX);
        let devices = env.new_object_array(len, "java/lang/String", JObject::null())?;
        for (i, name) in (0..len).zip(&notice.devices) {
            let name = env.new_string(name)?;
            env.set_object_array_element(&devices, i, name)?;
        }
        jni::call_static(
            env,
            context,
            "Service",
            "keepRunning",
            "(Landroid/content/Context;[Ljava/lang/String;Z)V",
            &[context.into(), (&devices).into(), notice.paused.into()],
        )?
        .v()
    })
}

/// Whether the user paused linking from the notification. Unreadable
/// counts as not paused: links stay as the user last had them.
pub(super) fn links_paused() -> bool {
    jni::call("Service.paused", |env, context| {
        jni::call_static(env, context, "Service", "paused", "()Z", &[])?.z()
    })
    .unwrap_or(false)
}
