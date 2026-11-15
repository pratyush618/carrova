package org.byteveda.carrova.platform

import android.content.Context
import android.net.wifi.WifiManager

/** Network glue for `carrova-platform` (android/net.rs). Rust releases the lock. */
object Net {
    @JvmStatic
    fun acquireMulticast(context: Context): WifiManager.MulticastLock {
        val wifi = context.applicationContext.getSystemService(Context.WIFI_SERVICE) as WifiManager
        return wifi.createMulticastLock("carrova").apply {
            setReferenceCounted(false)
            acquire()
        }
    }
}
