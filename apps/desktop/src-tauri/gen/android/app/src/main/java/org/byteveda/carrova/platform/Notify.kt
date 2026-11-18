package org.byteveda.carrova.platform

import android.app.ActivityOptions
import android.app.Notification
import android.app.PendingIntent
import android.app.RemoteInput
import android.content.Context
import android.content.Intent
import android.content.pm.ApplicationInfo
import android.content.pm.PackageManager
import android.graphics.Bitmap
import android.graphics.Canvas
import android.os.Build
import android.os.Bundle
import android.provider.Telephony
import android.service.notification.StatusBarNotification
import org.json.JSONArray
import org.json.JSONObject
import java.io.ByteArrayOutputStream

/** Notification glue for `carrova-platform` (android/phone.rs). Filtering beyond [shown] is Rust's. */
object Notify {
    /** Edge of the app icon sent with a note, in pixels. */
    private const val ICON_PX = 48

    /**
     * The posting app's [ApplicationInfo], which the system stamps on every
     * note (`Notification.EXTRA_BUILDER_APPLICATION_INFO`, hidden; what
     * `Notification.Builder.recoverBuilder` reads).
     */
    private const val EXTRA_APP_INFO = "android.appInfo"

    /** A system app's stand-in name (`Notification.EXTRA_SUBSTITUTE_APP_NAME`, system API). */
    private const val EXTRA_SUBSTITUTE_NAME = "android.substName"

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
        val pm = context.packageManager
        val app = appInfo(pm, sbn)
        val label = app?.let(pm::getApplicationLabel)
        val note = JSONObject()
            .put("key", sbn.key)
            .put("app_id", sbn.packageName)
            .put("app_name", appName(extras.getCharSequence(EXTRA_SUBSTITUTE_NAME), label, sbn.packageName))
            .put("title", extras.getCharSequence(Notification.EXTRA_TITLE)?.toString() ?: "")
            .put("text", text?.toString() ?: "")
            .put("when_ms", sbn.postTime)
            .put("can_reply", replyAction(sbn) != null)
            // Needs the SMS_DELIVER `<queries>` entry (package visibility); no permission.
            .put("sms_app", sbn.packageName == Telephony.Sms.getDefaultSmsPackage(context))
        app?.let { icon(pm, it) }?.let { png ->
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

    /** The name a note shows: the app's stand-in name, else its label, else (last resort) [pkg]. */
    fun appName(substitute: CharSequence?, label: CharSequence?, pkg: String): String =
        listOf(substitute, label).firstOrNull { !it.isNullOrBlank() }?.toString()?.trim() ?: pkg

    /**
     * The posting app, from the note itself: since Android 11 package
     * visibility hides other apps from a `PackageManager` lookup, and the
     * note's own copy needs neither `<queries>` nor `QUERY_ALL_PACKAGES`.
     */
    private fun appInfo(pm: PackageManager, sbn: StatusBarNotification): ApplicationInfo? {
        val extras = sbn.notification.extras
        val stamped = if (Build.VERSION.SDK_INT >= 33) {
            extras.getParcelable(EXTRA_APP_INFO, ApplicationInfo::class.java)
        } else {
            @Suppress("DEPRECATION")
            extras.getParcelable(EXTRA_APP_INFO)
        }
        return stamped ?: try {
            pm.getApplicationInfo(sbn.packageName, 0)
        } catch (e: PackageManager.NameNotFoundException) {
            null
        }
    }

    /** The app's icon as a small PNG. */
    private fun icon(pm: PackageManager, app: ApplicationInfo): ByteArray {
        val drawable = pm.getApplicationIcon(app)
        val bitmap = Bitmap.createBitmap(ICON_PX, ICON_PX, Bitmap.Config.ARGB_8888)
        drawable.setBounds(0, 0, ICON_PX, ICON_PX)
        drawable.draw(Canvas(bitmap))
        val png = ByteArrayOutputStream()
        bitmap.compress(Bitmap.CompressFormat.PNG, 100, png)
        bitmap.recycle()
        return png.toByteArray()
    }
}
