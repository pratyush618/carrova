package org.byteveda.carrova

import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.service.quicksettings.TileService
import org.byteveda.Carrova.MainActivity

/**
 * Quick-settings tile "Send clipboard": opens the app with [SEND_CLIPBOARD]. Android lets only
 * the focused app read the clipboard, so the app's UI sends it (to the computers Rust links).
 */
class ClipboardTile : TileService() {
    override fun onClick() {
        super.onClick()
        val intent = sendClipboardIntent(this)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            startActivityAndCollapse(PendingIntent.getActivity(this, 0, intent, PendingIntent.FLAG_IMMUTABLE))
        } else {
            @Suppress("DEPRECATION")
            startActivityAndCollapse(intent)
        }
    }

    companion object {
        /** The link the app is opened with (tao hands it to Rust as `RunEvent::Opened`). */
        const val SEND_CLIPBOARD = "carrova://send-clipboard"

        /** Opens the app with [SEND_CLIPBOARD] (tile and the service notification's action). */
        fun sendClipboardIntent(context: Context): Intent =
            Intent(Intent.ACTION_VIEW, Uri.parse(SEND_CLIPBOARD), context, MainActivity::class.java)
                .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
    }
}
