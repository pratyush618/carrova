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
 * in it, alive without the app's window, also after boot (ADR 0003, lifetime mechanism 2).
 * No decisions here: what links carry is decided in Rust.
 */
class LinkService : Service() {
    override fun onCreate() {
        super.onCreate()
        startSupervisor(applicationContext)
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        // Every start must promote the service within seconds.
        startForeground(NOTIFICATION_ID, notification(), ServiceInfo.FOREGROUND_SERVICE_TYPE_CONNECTED_DEVICE)
        return START_STICKY
    }

    override fun onBind(intent: Intent?): IBinder? = null

    private fun notification(): Notification {
        val channel = NotificationChannel(
            CHANNEL_ID, getString(R.string.link_channel), NotificationManager.IMPORTANCE_LOW,
        )
        getSystemService(NotificationManager::class.java).createNotificationChannel(channel)
        val icon = Icon.createWithResource(this, R.mipmap.ic_launcher)
        val sendClipboard = Notification.Action.Builder(
            icon,
            getString(R.string.send_clipboard),
            activity(ClipboardTile.sendClipboardIntent(this)),
        ).build()
        return Notification.Builder(this, CHANNEL_ID)
            .setSmallIcon(icon)
            .setContentTitle(getString(R.string.app_name))
            .setContentText(getString(R.string.link_running))
            .setContentIntent(activity(Intent(this, MainActivity::class.java)))
            .addAction(sendClipboard)
            .setOngoing(true)
            .build()
    }

    private fun activity(intent: Intent): PendingIntent = PendingIntent.getActivity(
        this, intent.hashCode(), intent, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
    )

    /**
     * Rust (src-tauri `mobile.rs`): keeps [context] for platform calls, then starts the link
     * supervisor, once per process (the app's own start shares the same guard).
     */
    private external fun startSupervisor(context: Context)

    companion object {
        private const val CHANNEL_ID = "link"
        private const val NOTIFICATION_ID = 1

        init {
            System.loadLibrary("carrova_desktop")
        }

        /** Start (or keep) the service. */
        fun start(context: Context) {
            context.startForegroundService(Intent(context, LinkService::class.java))
        }
    }
}
