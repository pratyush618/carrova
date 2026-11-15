//! Network glue through `Net.kt`: the Wi-Fi multicast lock (Android
//! drops multicast, so mDNS, unless an app holds one) and whether the
//! active network is a local one.

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

/// Whether the active network is Wi-Fi or Ethernet (not mobile data, not none).
pub(super) fn on_local_network() -> Result<bool, PlatformError> {
    jni::call("Net.onLocalNetwork", |env, context| {
        jni::call_static(
            env,
            context,
            "Net",
            "onLocalNetwork",
            "(Landroid/content/Context;)Z",
            &[context.into()],
        )?
        .z()
    })
}
