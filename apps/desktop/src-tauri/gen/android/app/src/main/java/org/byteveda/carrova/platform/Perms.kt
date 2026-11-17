package org.byteveda.carrova.platform

import android.Manifest
import android.app.AppOpsManager
import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.os.PowerManager
import android.os.Process
import android.provider.Settings
import androidx.core.app.NotificationManagerCompat
import androidx.core.content.ContextCompat
import java.util.concurrent.TimeUnit
import org.byteveda.Carrova.R

/**
 * Permission glue for `carrova-platform` (android/phone.rs). Names are
 * `phone::PhonePerm`'s; states `granted`, `denied` or `restricted`.
 */
object Perms {
    /** Status of permission [name]. */
    @JvmStatic
    fun state(context: Context, name: String): String {
        val perms = runtime(name) // rejects unknown names
        return stateOf(
            granted = {
                if (name == "notification_access") {
                    NotificationManagerCompat.getEnabledListenerPackages(context).contains(context.packageName)
                } else if (name == "battery") {
                    context.getSystemService(PowerManager::class.java)
                        .isIgnoringBatteryOptimizations(context.packageName)
                } else {
                    perms.all { ContextCompat.checkSelfPermission(context, it) == PackageManager.PERMISSION_GRANTED }
                }
            },
            restricted = { restrictable(name) && restricted(context) },
        )
    }

    /**
     * `granted`, `restricted` or `denied`. A check this Android hides from
     * apps (some OEM builds throw SecurityException) or doesn't know reads
     * as no, so the user still gets a step that opens the settings page.
     */
    internal fun stateOf(granted: () -> Boolean, restricted: () -> Boolean): String = when {
        answers(granted) -> "granted"
        answers(restricted) -> "restricted"
        else -> "denied"
    }

    private fun answers(check: () -> Boolean): Boolean = try {
        check()
    } catch (e: SecurityException) {
        false
    } catch (e: IllegalArgumentException) {
        false
    }

    /**
     * Ask for runtime permission [name] with Android's own dialog and return its state once
     * answered. Blocks (called off the main thread); throws for names granted on a settings page.
     */
    @JvmStatic
    fun request(context: Context, name: String): String {
        val perms = runtime(name) // rejects unknown names
        require(name != "notification_access" && name != "battery") { "not a runtime permission" }
        if (state(context, name) == "granted") return "granted"
        val latch = checkNotNull(PermRequest.begin()) { "a permission request is open" }
        try {
            context.startActivity(
                Intent(context, PermRequestActivity::class.java)
                    .putExtra(PermRequestActivity.PERMS, perms)
                    .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK),
            )
            latch.await(REQUEST_MINUTES, TimeUnit.MINUTES)
        } finally {
            PermRequest.end(latch)
        }
        return state(context, name)
    }

    /** A forgotten dialog gives up after this long. */
    private const val REQUEST_MINUTES = 2L

    /** Open the settings page where [name] is granted (App info while restricted). */
    @JvmStatic
    fun openSettings(context: Context, name: String) {
        context.startActivity(settings(context, name))
    }

    /**
     * Post a notification asking the user to grant [name]; a tap opens [openSettings]' page.
     * Throws if this app may not post notifications, so the asking device hears it failed.
     */
    @JvmStatic
    fun remind(context: Context, name: String) {
        check(NotificationManagerCompat.from(context).areNotificationsEnabled()) { "notifications off" }
        val manager = context.getSystemService(NotificationManager::class.java)
        manager.createNotificationChannel(
            NotificationChannel(REMIND_CHANNEL, context.getString(R.string.remind_channel), NotificationManager.IMPORTANCE_DEFAULT),
        )
        val tap = PendingIntent.getActivity(
            context, name.hashCode(), settings(context, name),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )
        val title = if (name == "notification_access") R.string.remind_notifications else R.string.remind_sms
        val notice = Notification.Builder(context, REMIND_CHANNEL)
            .setSmallIcon(R.drawable.ic_stat_link)
            .setContentTitle(context.getString(title))
            .setContentText(context.getString(R.string.remind_text))
            .setContentIntent(tap)
            .setAutoCancel(true)
            .build()
        manager.notify(REMIND_TAG, name.hashCode(), notice)
    }

    private const val REMIND_CHANNEL = "access"
    private const val REMIND_TAG = "remind"

    private fun settings(context: Context, name: String): Intent {
        runtime(name) // rejects unknown names
        val intent = if (name == "notification_access" && !answers { restricted(context) }) {
            Intent(Settings.ACTION_NOTIFICATION_LISTENER_SETTINGS)
        } else if (name == "battery") {
            // Android's own "Let app always run in background?" dialog.
            @Suppress("BatteryLife")
            Intent(Settings.ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS, Uri.fromParts("package", context.packageName, null))
        } else {
            Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS, Uri.fromParts("package", context.packageName, null))
        }
        return intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
    }

    private fun runtime(name: String): Array<String> = when (name) {
        "notification_access", "battery" -> emptyArray()
        "sms" -> arrayOf(Manifest.permission.READ_SMS, Manifest.permission.SEND_SMS, Manifest.permission.RECEIVE_SMS)
        "calls" -> arrayOf(
            Manifest.permission.READ_PHONE_STATE, Manifest.permission.READ_CALL_LOG,
            Manifest.permission.ANSWER_PHONE_CALLS, Manifest.permission.CALL_PHONE,
        )
        "contacts" -> arrayOf(Manifest.permission.READ_CONTACTS)
        "bluetooth_connect" ->
            if (Build.VERSION.SDK_INT >= 31) arrayOf(Manifest.permission.BLUETOOTH_CONNECT) else emptyArray()
        else -> throw IllegalArgumentException("unknown permission")
    }

    /** The permissions Android 13+ holds back from sideloaded apps (spec S3). */
    private fun restrictable(name: String) = name == "notification_access" || name == "sms" || name == "calls"

    /** Whether "Allow restricted settings" is still off for this app. */
    @Suppress("DEPRECATION")
    private fun restricted(context: Context): Boolean {
        if (Build.VERSION.SDK_INT < 33) return false
        val ops = context.getSystemService(AppOpsManager::class.java)
        return ops.unsafeCheckOpNoThrow("android:access_restricted_settings", Process.myUid(), context.packageName) !=
            AppOpsManager.MODE_ALLOWED
    }
}
