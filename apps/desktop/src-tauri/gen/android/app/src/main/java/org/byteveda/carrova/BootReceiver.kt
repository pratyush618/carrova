package org.byteveda.carrova

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent

/**
 * Starts [LinkService] after boot and after an app update; Rust stops it at once if no device
 * is linked. An update force-stops the app, which drops the running service with it, and
 * Android may start the process again for another reason (the notification listener) that
 * starts no link. Both broadcasts may start a foreground service from the background.
 */
class BootReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        if (startsLink(intent.action)) {
            LinkService.start(context)
        }
    }

    companion object {
        /** Whether the broadcast [action] starts the link service. */
        fun startsLink(action: String?): Boolean =
            action == Intent.ACTION_BOOT_COMPLETED || action == Intent.ACTION_MY_PACKAGE_REPLACED
    }
}
