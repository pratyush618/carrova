package org.byteveda.carrova

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent

/** Starts [LinkService] after boot, so linked devices reconnect without opening the app. */
class BootReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        if (intent.action == Intent.ACTION_BOOT_COMPLETED) {
            LinkService.start(context)
        }
    }
}
