package org.byteveda.carrova.platform

import android.app.ActivityOptions
import android.app.Notification
import android.app.PendingIntent
import android.app.RemoteInput
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.graphics.Bitmap
import android.graphics.Canvas
import android.os.Build
import android.os.Bundle
import android.service.notification.StatusBarNotification
import org.json.JSONArray
import org.json.JSONObject
import java.io.ByteArrayOutputStream

/** Notification glue for `carrova-platform` (android/phone.rs). Filtering beyond [shown] is Rust's. */
object Notify {
    /** Edge of the app icon sent with a note, in pixels. */
    private const val ICON_PX = 48

    /**
     * Whether a note leaves the phone at all: not ongoing (media, calls,
     * services), not a group summary (its children are sent) and not Carrova's own.
     */
    fun shown(context: Context, sbn: StatusBarNotification): Boolean {
        val flags = sbn.notification.flags
        return flags and Notification.FLAG_ONGOING_EVENT == 0 &&
            flags and Notification.FLAG_GROUP_SUMMARY == 0 &&
            sbn.packageName != context.packageName
    }

    /** A note's fields as `phone::Note` reads them. */
    fun json(context: Context, sbn: StatusBarNotification): JSONObject {
        val extras = sbn.notification.extras
        val text = extras.getCharSequence(Notification.EXTRA_BIG_TEXT)
            ?: extras.getCharSequence(Notification.EXTRA_TEXT)
        val note = JSONObject()
            .put("key", sbn.key)
            .put("app_id", sbn.packageName)
            .put("app_name", appName(context, sbn.packageName))
            .put("title", extras.getCharSequence(Notification.EXTRA_TITLE)?.toString() ?: "")
            .put("text", text?.toString() ?: "")
            .put("when_ms", sbn.postTime)
            .put("can_reply", replyAction(sbn) != null)
        icon(context, sbn.packageName)?.let { png ->
            note.put("icon", JSONArray().apply { png.forEach { put(it.toInt() and 0xff) } })
        }
        return note
    }

    /** Notes shown now, as a JSON array. */
    @JvmStatic
    fun active(context: Context): String {
        val notes = JSONArray()
        listener().activeNotifications.filter { shown(context, it) }.forEach { notes.put(json(context, it)) }
        return notes.toString()
    }

    /** Reply inline to note [key]; false when it is gone or takes no reply. */
    @JvmStatic
    fun reply(context: Context, key: String, text: String): Boolean {
        val action = find(key)?.let(::replyAction) ?: return false
        val inputs = action.remoteInputs
        val results = Bundle().apply {
            inputs.filter { it.allowFreeFormInput }.forEach { putCharSequence(it.resultKey, text) }
        }
        val intent = Intent()
        RemoteInput.addResultsToIntent(inputs, intent, results)
        return try {
            action.actionIntent.send(context, 0, intent)
            true
        } catch (e: PendingIntent.CanceledException) {
            false
        }
    }

    /** Dismiss note [key]; a gone one is no error. */
    @JvmStatic
    fun dismiss(context: Context, key: String) {
        listener().cancelNotification(key)
    }

    /** Open note [key] as a tap would; a gone one does nothing. */
    @JvmStatic
    fun open(context: Context, key: String) {
        val intent = find(key)?.notification?.contentIntent ?: return
        val options = if (Build.VERSION.SDK_INT >= 34) {
            @Suppress("DEPRECATION")
            ActivityOptions.makeBasic()
                .setPendingIntentBackgroundActivityStartMode(ActivityOptions.MODE_BACKGROUND_ACTIVITY_START_ALLOWED)
                .toBundle()
        } else {
            null
        }
        try {
            intent.send(context, 0, null, null, null, null, options)
        } catch (e: PendingIntent.CanceledException) {
            // Gone meanwhile.
        }
    }

    private fun listener(): NotifyListener =
        NotifyListener.instance ?: throw IllegalStateException("no notification access")

    private fun find(key: String): StatusBarNotification? =
        listener().getActiveNotifications(arrayOf(key))?.firstOrNull()

    private fun replyAction(sbn: StatusBarNotification): Notification.Action? =
        sbn.notification.actions?.firstOrNull { action ->
            action.remoteInputs?.any { it.allowFreeFormInput } == true
        }

    private fun appName(context: Context, pkg: String): String {
        val pm = context.packageManager
        return try {
            pm.getApplicationLabel(pm.getApplicationInfo(pkg, 0)).toString()
        } catch (e: PackageManager.NameNotFoundException) {
            pkg
        }
    }

    /** The app's icon as a small PNG; null if it cannot be had. */
    private fun icon(context: Context, pkg: String): ByteArray? {
        val drawable = try {
            context.packageManager.getApplicationIcon(pkg)
        } catch (e: PackageManager.NameNotFoundException) {
            return null
        }
        val bitmap = Bitmap.createBitmap(ICON_PX, ICON_PX, Bitmap.Config.ARGB_8888)
        drawable.setBounds(0, 0, ICON_PX, ICON_PX)
        drawable.draw(Canvas(bitmap))
        val png = ByteArrayOutputStream()
        bitmap.compress(Bitmap.CompressFormat.PNG, 100, png)
        bitmap.recycle()
        return png.toByteArray()
    }
}
