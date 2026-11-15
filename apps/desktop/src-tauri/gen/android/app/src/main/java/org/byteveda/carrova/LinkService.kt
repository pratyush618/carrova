package org.byteveda.carrova

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.graphics.drawable.Icon
import android.os.IBinder
import org.byteveda.Carrova.MainActivity
import org.byteveda.Carrova.R

/**
 * Foreground service (`connectedDevice`) that keeps the process, and the Rust link supervisor
 * in it, alive without the app's window (ADR 0003, lifetime mechanism 2). Rust starts, updates
 * and stops it (`Platform::keep_running`): while some device is linked, and after boot only if
 * one is. No decisions here; Pause/Resume only record the tap, which Rust reads.
 */
class LinkService : Service() {
    /** What Rust last asked the notification to show. */
    data class Notice(val devices: List<String>, val paused: Boolean)

    override fun onCreate() {
        super.onCreate()
        running = this
        startSupervisor(applicationContext)
    }

    override fun onDestroy() {
        running = null
        super.onDestroy()
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_PAUSE -> pauseTapped = true
            ACTION_RESUME -> pauseTapped = false
        }
        // Every start must promote the service within seconds.
        startForeground(NOTIFICATION_ID, notification(), ServiceInfo.FOREGROUND_SERVICE_TYPE_CONNECTED_DEVICE)
        return START_STICKY
    }

    override fun onBind(intent: Intent?): IBinder? = null

    private fun refresh() {
        getSystemService(NotificationManager::class.java).notify(NOTIFICATION_ID, notification())
    }

    private fun notification(): Notification {
        val channel = NotificationChannel(
            CHANNEL_ID, getString(R.string.link_channel), NotificationManager.IMPORTANCE_LOW,
        )
        getSystemService(NotificationManager::class.java).createNotificationChannel(channel)
        val shown = notice
        val text = when {
            shown == null -> getString(R.string.link_starting)
            shown.paused -> getString(R.string.link_paused)
            else -> getString(R.string.link_running, shown.devices.joinToString(", "))
        }
        val builder = Notification.Builder(this, CHANNEL_ID)
            .setSmallIcon(R.mipmap.ic_launcher)
            .setContentTitle(getString(R.string.app_name))
            .setContentText(text)
            .setContentIntent(activity(Intent(this, MainActivity::class.java)))
            .setOngoing(true)
        if (shown?.paused == true) {
            builder.addAction(action(R.string.resume, service(ACTION_RESUME)))
        } else {
            builder.addAction(action(R.string.send_clipboard, activity(ClipboardTile.sendClipboardIntent(this))))
            builder.addAction(action(R.string.pause, service(ACTION_PAUSE)))
        }
        return builder.build()
    }

    private fun action(title: Int, intent: PendingIntent): Notification.Action =
        Notification.Action.Builder(Icon.createWithResource(this, R.mipmap.ic_launcher), getString(title), intent)
            .build()

    private fun activity(intent: Intent): PendingIntent = PendingIntent.getActivity(
        this, intent.hashCode(), intent, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
    )

    private fun service(action: String): PendingIntent = PendingIntent.getService(
        this, action.hashCode(), Intent(this, LinkService::class.java).setAction(action),
        PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
    )

    /**
     * Rust (src-tauri `mobile.rs`): keeps [context] for platform calls, then starts the link
     * supervisor, once per process (the app's own start shares the same guard).
     */
    private external fun startSupervisor(context: Context)

    companion object {
        private const val CHANNEL_ID = "link"
        private const val NOTIFICATION_ID = 1
        private const val ACTION_PAUSE = "org.byteveda.carrova.PAUSE"
        private const val ACTION_RESUME = "org.byteveda.carrova.RESUME"

        @Volatile private var running: LinkService? = null
        @Volatile private var notice: Notice? = null

        /** The user's last Pause/Resume tap (read by Rust through `Service.paused`). */
        @Volatile var pauseTapped = false
            private set

        init {
            System.loadLibrary("carrova_desktop")
        }

        /** Start the service (boot), before Rust has said what it shows. */
        fun start(context: Context) {
            context.startForegroundService(Intent(context, LinkService::class.java))
        }

        /** Show [shown]: update the running service's notification, or start the service. */
        fun show(context: Context, shown: Notice) {
            notice = shown
            val service = running
            if (service != null) service.refresh() else start(context)
        }
    }
}
