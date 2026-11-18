package org.byteveda.carrova.platform

import android.content.Context
import android.net.ConnectivityManager
import android.net.NetworkCapabilities
import android.net.wifi.WifiManager
import android.os.Build
import android.provider.Settings

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

    /** The name the user gave this phone (Settings › About phone), else its model. */
    @JvmStatic
    fun deviceName(context: Context): String {
        // Not every build sets it, and some hide it from apps.
        val named = try {
            Settings.Global.getString(context.contentResolver, Settings.Global.DEVICE_NAME)
        } catch (e: SecurityException) {
            null
        }
        return named?.takeIf { it.isNotBlank() } ?: Build.MODEL
    }
}
