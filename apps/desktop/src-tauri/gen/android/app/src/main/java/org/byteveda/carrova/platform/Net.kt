package org.byteveda.carrova.platform

import android.content.Context
import android.net.ConnectivityManager
import android.net.NetworkCapabilities
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

    /** Whether the active network is Wi-Fi or Ethernet. */
    @JvmStatic
    fun onLocalNetwork(context: Context): Boolean {
        val connectivity = context.applicationContext.getSystemService(ConnectivityManager::class.java)
            ?: return false
        val caps = connectivity.getNetworkCapabilities(connectivity.activeNetwork) ?: return false
        return caps.hasTransport(NetworkCapabilities.TRANSPORT_WIFI) ||
            caps.hasTransport(NetworkCapabilities.TRANSPORT_ETHERNET)
    }
}
