//! The Wi-Fi multicast lock through `Net.kt`: Android drops multicast
//! (mDNS) unless an app holds one.

use super::jni;
use crate::{MulticastGuard, PlatformError};

/// Acquire a multicast lock, released when the guard drops.
pub(super) fn multicast_lock() -> Result<MulticastGuard, PlatformError> {
    let lock = jni::call("Net.acquireMulticast", |env, context| {
        let lock = jni::call_static(
            env,
            context,
            "Net",
            "acquireMulticast",
            "(Landroid/content/Context;)Landroid/net/wifi/WifiManager$MulticastLock;",
            &[context.into()],
        )?
        .l()?;
        env.new_global_ref(lock)
    })?;
    Ok(MulticastGuard::held(move || {
        // Nothing to report to from a drop; the OS frees it with the process.
        let _ = jni::call("MulticastLock.release", |env, _| {
            env.call_method(&lock, "release", "()V", &[])?.v()
        });
    }))
}
