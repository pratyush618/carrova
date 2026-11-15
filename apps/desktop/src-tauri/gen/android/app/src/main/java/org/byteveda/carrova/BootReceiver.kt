package org.byteveda.carrova

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent

/** Starts [LinkService] after boot; Rust stops it at once if no device is linked. */
class BootReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        if (intent.action == Intent.ACTION_BOOT_COMPLETED) {
            LinkService.start(context)
        }
    }
}
