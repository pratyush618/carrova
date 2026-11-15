//! Ring through `Ring.kt`: the alarm stream at full volume, so it sounds
//! with the phone on silent.

use super::jni;
use crate::PlatformError;

/// Ring for `seconds` (a new ring replaces a playing one).
pub(super) fn ring(seconds: u16) -> Result<(), PlatformError> {
    jni::call("Ring.start", |env, context| {
        jni::call_static(
            env,
            context,
            "Ring",
            "start",
            "(Landroid/content/Context;I)V",
            &[context.into(), i32::from(seconds).into()],
        )?
        .v()
    })
}

/// Stop a ring; nothing to do if none plays.
pub(super) fn stop_ring() -> Result<(), PlatformError> {
    jni::call("Ring.stop", |env, context| {
        jni::call_static(
            env,
            context,
            "Ring",
            "stop",
            "(Landroid/content/Context;)V",
            &[context.into()],
        )?
        .v()
    })
}
