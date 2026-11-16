package org.byteveda.carrova.platform

import android.service.notification.NotificationListenerService
import android.service.notification.StatusBarNotification
import org.json.JSONObject

/** Notification access: posts and removals go to [Events]; [Notify] acts through [instance]. */
class NotifyListener : NotificationListenerService() {
    override fun onListenerConnected() {
        instance = this
    }

    override fun onListenerDisconnected() {
        instance = null
    }

    override fun onNotificationPosted(sbn: StatusBarNotification) {
        if (Events.watching && Notify.shown(this, sbn)) {
            Events.offer(Notify.json(this, sbn).put("type", "note_posted").toString())
        }
    }

    override fun onNotificationRemoved(sbn: StatusBarNotification) {
        if (Events.watching && Notify.shown(this, sbn)) {
            Events.offer(JSONObject().put("type", "note_removed").put("key", sbn.key).toString())
        }
    }

    companion object {
        @Volatile
        var instance: NotifyListener? = null
            private set
    }
}
