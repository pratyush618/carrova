package org.byteveda.carrova.platform

import android.content.Context
import android.content.Intent
import org.byteveda.carrova.LinkService

/** Link service glue for `carrova-platform` (android/service.rs). Rust decides when it runs. */
object Service {
    /** Run [LinkService] showing [devices] (and whether linking is [paused]). */
    @JvmStatic
    fun keepRunning(context: Context, devices: Array<String>, paused: Boolean) {
        LinkService.show(context, LinkService.Notice(devices.toList(), paused))
    }

    @JvmStatic
    fun stop(context: Context) {
        context.stopService(Intent(context, LinkService::class.java))
    }

    /** Whether the user tapped Pause (and not Resume since) in the notification. */
    @JvmStatic
    fun paused(): Boolean = LinkService.pauseTapped
}
