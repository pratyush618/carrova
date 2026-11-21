package org.byteveda.carrova.platform

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.pm.PackageInfo
import android.content.pm.PackageManager
import android.graphics.drawable.Icon
import android.net.Uri
import android.os.Build
import android.provider.Settings
import android.text.format.Formatter
import androidx.core.content.FileProvider
import org.byteveda.Carrova.MainActivity
import org.byteveda.Carrova.R
import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import java.security.MessageDigest

/**
 * App update glue for `carrova-platform` (android/update.rs): read a package's facts, open
 * Android's install permission and installer, show the download. Whether an APK is trusted,
 * and when to download or install, is Rust's.
 */
object Updates {
    private const val CHANNEL_ID = "update"
    private const val NOTIFICATION_ID = 2
    private const val APK_MIME = "application/vnd.android.package-archive"

    /** Download stages as Rust sends them (`UpdateStage`). */
    private const val DOWNLOADING = 0
    private const val PAUSED = 1

    /** The facts of the APK at [path] (JSON, as `AppPackage` reads it). */
    @JvmStatic
    fun inspect(context: Context, path: String): String {
        val pm = context.packageManager
        val info = if (Build.VERSION.SDK_INT >= 33) {
            pm.getPackageArchiveInfo(path, PackageManager.PackageInfoFlags.of(PackageManager.GET_SIGNING_CERTIFICATES.toLong()))
        } else {
            @Suppress("DEPRECATION")
            pm.getPackageArchiveInfo(path, PackageManager.GET_SIGNING_CERTIFICATES)
        } ?: throw IllegalStateException("not a readable APK")
        return json(info)
    }

    /** The facts of this app as installed (JSON, as `AppPackage` reads it). */
    @JvmStatic
    fun installed(context: Context): String {
        val pm = context.packageManager
        val info = if (Build.VERSION.SDK_INT >= 33) {
            pm.getPackageInfo(context.packageName, PackageManager.PackageInfoFlags.of(PackageManager.GET_SIGNING_CERTIFICATES.toLong()))
        } else {
            @Suppress("DEPRECATION")
            pm.getPackageInfo(context.packageName, PackageManager.GET_SIGNING_CERTIFICATES)
        }
        return json(info)
    }

    @JvmStatic
    fun canInstall(context: Context): Boolean = context.packageManager.canRequestPackageInstalls()

    /** Android's "Install unknown apps" page, opened on this app. */
    @JvmStatic
    fun openInstallSettings(context: Context) {
        val intent = Intent(Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES, Uri.parse("package:${context.packageName}"))
        context.startActivity(intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
    }

    /** Hand the APK at [path] to Android's installer, which asks the user before it installs. */
    @JvmStatic
    fun install(context: Context, path: String) {
        val uri = FileProvider.getUriForFile(context, "${context.packageName}.fileprovider", File(path))
        val intent = Intent(Intent.ACTION_VIEW)
            .setDataAndType(uri, APK_MIME)
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_GRANT_READ_URI_PERMISSION)
        context.startActivity(intent)
    }

    /**
     * Show the download of [version] as an ongoing notification: [stage] (downloading, paused
     * or waiting for Wi-Fi), [done] of [total] bytes (-1: unknown) at [rate] bytes a second
     * (-1: not measured yet).
     */
    @JvmStatic
    fun show(context: Context, version: String, stage: Int, done: Long, total: Long, rate: Long) {
        val manager = context.getSystemService(NotificationManager::class.java)
        manager.createNotificationChannel(
            NotificationChannel(CHANNEL_ID, context.getString(R.string.update_channel), NotificationManager.IMPORTANCE_LOW),
        )
        val size = { bytes: Long -> Formatter.formatShortFileSize(context, bytes) }
        val got = if (total < 0) size(done) else context.getString(R.string.update_of, size(done), size(total))
        val text = when (stage) {
            DOWNLOADING -> if (rate < 0) got else context.getString(R.string.update_rate, got, size(rate))
            PAUSED -> context.getString(R.string.update_paused, got)
            else -> context.getString(R.string.update_waiting, got)
        }
        val percent = if (total > 0) (done * 100 / total).toInt() else 0
        val builder = Notification.Builder(context, CHANNEL_ID)
            .setSmallIcon(R.drawable.ic_stat_link)
            .setContentTitle(context.getString(R.string.update_title, version))
            .setContentText(text)
            .setProgress(100, percent, total < 0)
            .setContentIntent(
                PendingIntent.getActivity(
                    context, 0, Intent(context, MainActivity::class.java), PendingIntent.FLAG_IMMUTABLE,
                ),
            )
            .setOngoing(true)
            .setOnlyAlertOnce(true)
        when (stage) {
            DOWNLOADING -> builder.addAction(action(context, R.string.pause, UpdateActions.PAUSE))
            PAUSED -> builder.addAction(action(context, R.string.resume, UpdateActions.RESUME))
        }
        builder.addAction(action(context, R.string.cancel, UpdateActions.CANCEL))
        manager.notify(NOTIFICATION_ID, builder.build())
    }

    @JvmStatic
    fun hide(context: Context) {
        context.getSystemService(NotificationManager::class.java).cancel(NOTIFICATION_ID)
    }

    /** The user's last tap on the notification (Pause, Resume or Cancel), taken once; null if none. */
    @JvmStatic
    fun takeTap(): String? = UpdateActions.take()

    /** SHA-256 of [bytes], as unsigned byte values (a JSON array Rust reads as `[u8; 32]`). */
    fun sha256(bytes: ByteArray): List<Int> =
        MessageDigest.getInstance("SHA-256").digest(bytes).map { it.toInt() and 0xff }

    private fun json(info: PackageInfo): String {
        val signers = JSONArray()
        info.signingInfo?.apkContentsSigners?.forEach { signers.put(JSONArray(sha256(it.toByteArray()))) }
        return JSONObject()
            .put("package", info.packageName)
            .put("version_code", info.longVersionCode)
            .put("signers", signers)
            .toString()
    }

    private fun action(context: Context, title: Int, tap: String): Notification.Action {
        val intent = Intent(context, UpdateActions::class.java).setAction(tap)
        val pending = PendingIntent.getBroadcast(
            context, tap.hashCode(), intent, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )
        return Notification.Action.Builder(Icon.createWithResource(context, R.drawable.ic_stat_link), context.getString(title), pending)
            .build()
    }
}

/** Records the update notification's Pause, Resume and Cancel taps; Rust reads them (`Updates.takeTap`). */
class UpdateActions : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        tap(intent.action)
    }

    companion object {
        const val PAUSE = "org.byteveda.carrova.UPDATE_PAUSE"
        const val RESUME = "org.byteveda.carrova.UPDATE_RESUME"
        const val CANCEL = "org.byteveda.carrova.UPDATE_CANCEL"

        private var tapped: String? = null

        /** Record the tap of notification action [action] (unknown ones are ignored). */
        @Synchronized
        fun tap(action: String?) {
            tapped = when (action) {
                PAUSE -> "pause"
                RESUME -> "resume"
                CANCEL -> "cancel"
                else -> return
            }
        }

        @Synchronized
        fun take(): String? = tapped.also { tapped = null }
    }
}
