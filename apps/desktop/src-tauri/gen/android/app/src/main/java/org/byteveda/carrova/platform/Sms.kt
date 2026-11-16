package org.byteveda.carrova.platform

import android.Manifest
import android.app.Activity
import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.content.pm.PackageManager
import android.database.ContentObserver
import android.net.Uri
import android.os.Build
import android.os.Handler
import android.os.HandlerThread
import android.provider.ContactsContract
import android.provider.Telephony
import android.telephony.SmsManager
import androidx.core.content.ContextCompat
import org.json.JSONArray
import org.json.JSONObject
import java.util.UUID
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit

/**
 * SMS glue for `carrova-platform` (android/phone.rs): reads of the SMS
 * provider, sending through [SmsManager]. Pages are newest first by
 * `(date, id)`, exclusive of the `(beforeMs, beforeId)` pair.
 */
object Sms {
    private var observer: ContentObserver? = null
    private var worker: HandlerThread? = null

    /**
     * Tell [Events] when an incoming SMS is stored ([on]), or stop; a replaced
     * watch's [gen] does nothing. `SMS_RECEIVED` comes before the default
     * SMS app stores the message, so the laptop refetches on this instead.
     * Changes are debounced on the trailing edge (the last one is never lost).
     * A sent text or a mark-read also fires it: a harmless extra refetch.
     */
    @JvmStatic
    @Synchronized
    fun listen(context: Context, on: Boolean, gen: Long) {
        if (gen != Events.generation) return
        val resolver = context.contentResolver
        observer?.let { resolver.unregisterContentObserver(it) }
        worker?.quit()
        observer = null
        worker = null
        if (!on) return
        val thread = HandlerThread("carrova-sms-stored").also { it.start() }
        val handler = Handler(thread.looper)
        val debounce = Debounce(STORED_DEBOUNCE_MS, HandlerTimer(handler)) {
            newestInboxThread(resolver)?.let {
                Events.offer(JSONObject().put("type", "sms_stored").put("thread_id", it).toString())
            }
        }
        val next = object : ContentObserver(handler) {
            override fun onChange(selfChange: Boolean) = debounce.poke()
        }
        resolver.registerContentObserver(Telephony.Sms.CONTENT_URI, true, next)
        observer = next
        worker = thread
    }

    /** The thread of the newest inbox row; reads nothing else. Null without SMS permission. */
    private fun newestInboxThread(resolver: android.content.ContentResolver): Long? = try {
        resolver.query(
            Telephony.Sms.Inbox.CONTENT_URI, arrayOf(Telephony.Sms.THREAD_ID), null, null,
            "${Telephony.Sms.DATE} DESC, ${Telephony.Sms._ID} DESC",
        )?.use { if (it.moveToFirst()) it.getLong(0) else null }
    } catch (e: SecurityException) {
        null
    }

    /** How long a send may take before it counts as failed. */
    private const val SEND_TIMEOUT_S = 60L

    /** Quiet time after the last provider change before the newest thread is read. */
    private const val STORED_DEBOUNCE_MS = 500L
    private const val NOT_DRAFT = "${Telephony.Sms.TYPE} <> ${Telephony.Sms.MESSAGE_TYPE_DRAFT}"
    private const val NEWEST_FIRST = "${Telephony.Sms.DATE} DESC, ${Telephony.Sms._ID} DESC"

    /** Up to [limit] threads, by their latest message, before the pair and after [sinceMs]. */
    @JvmStatic
    fun threads(context: Context, beforeMs: Long, beforeId: Long, limit: Int, sinceMs: Long): String {
        val unread = HashMap<Long, Int>()
        context.contentResolver.query(
            Telephony.Sms.CONTENT_URI, arrayOf(Telephony.Sms.THREAD_ID),
            "${Telephony.Sms.READ} = 0 AND ${Telephony.Sms.TYPE} = ${Telephony.Sms.MESSAGE_TYPE_INBOX}",
            null, null,
        )?.use { c -> while (c.moveToNext()) unread.merge(c.getLong(0), 1, Int::plus) }

        val seen = HashSet<Long>()
        val found = ArrayList<JSONObject>()
        val projection = arrayOf(Telephony.Sms.THREAD_ID, Telephony.Sms.ADDRESS, Telephony.Sms.BODY, Telephony.Sms.DATE)
        context.contentResolver.query(
            Telephony.Sms.CONTENT_URI, projection,
            "${Telephony.Sms.DATE} > ? AND $NOT_DRAFT", arrayOf(sinceMs.toString()), NEWEST_FIRST,
        )?.use { c ->
            while (c.moveToNext()) {
                val thread = c.getLong(0)
                val date = c.getLong(3)
                // Past the limit, finish the run of equal dates so the sort below is exact.
                if (found.size >= limit && date < found.last().getLong("when_ms")) break
                if (!seen.add(thread)) continue
                if (date > beforeMs || (date == beforeMs && thread >= beforeId)) continue
                val address = c.getString(1) ?: ""
                found += JSONObject()
                    .put("id", thread)
                    .put("name", contactName(context, address) ?: JSONObject.NULL)
                    .put("address", address)
                    .put("snippet", c.getString(2) ?: "")
                    .put("when_ms", date)
                    .put("unread", unread[thread] ?: 0)
            }
        }
        found.sortWith(compareByDescending<JSONObject> { it.getLong("when_ms") }.thenByDescending { it.getLong("id") })
        return JSONArray(found.take(limit)).toString()
    }

    /** Up to [limit] messages of thread [id] before the pair. */
    @JvmStatic
    fun thread(context: Context, id: Long, beforeMs: Long, beforeId: Long, limit: Int): String {
        val messages = JSONArray()
        val projection = arrayOf(Telephony.Sms._ID, Telephony.Sms.TYPE, Telephony.Sms.BODY, Telephony.Sms.DATE)
        context.contentResolver.query(
            Telephony.Sms.CONTENT_URI, projection,
            "${Telephony.Sms.THREAD_ID} = ? AND $NOT_DRAFT AND " +
                "(${Telephony.Sms.DATE} < ? OR (${Telephony.Sms.DATE} = ? AND ${Telephony.Sms._ID} < ?))",
            arrayOf(id.toString(), beforeMs.toString(), beforeMs.toString(), beforeId.toString()),
            NEWEST_FIRST,
        )?.use { c ->
            while (messages.length() < limit && c.moveToNext()) {
                messages.put(
                    JSONObject()
                        .put("id", c.getLong(0))
                        .put("thread_id", id)
                        .put("outgoing", c.getInt(1) != Telephony.Sms.MESSAGE_TYPE_INBOX)
                        .put("text", c.getString(2) ?: "")
                        .put("when_ms", c.getLong(3)),
                )
            }
        }
        return messages.toString()
    }

    /**
     * Send [text] to [to]; waits for the radio: `sent`, `no_signal`, `failed`,
     * or `unknown` when it may have gone (no answer in time, or only some parts sent).
     */
    @JvmStatic
    fun send(context: Context, to: String, text: String): String {
        val sms = if (Build.VERSION.SDK_INT >= 31) {
            context.getSystemService(SmsManager::class.java)
        } else {
            @Suppress("DEPRECATION")
            SmsManager.getDefault()
        }
        val parts = sms.divideMessage(text)
        val action = "${context.packageName}.SMS_SENT.${UUID.randomUUID()}"
        val done = CountDownLatch(parts.size)
        val codes = ArrayList<Int>()
        val receiver = object : BroadcastReceiver() {
            override fun onReceive(context: Context, intent: Intent) {
                synchronized(codes) { codes += resultCode }
                done.countDown()
            }
        }
        ContextCompat.registerReceiver(context, receiver, IntentFilter(action), ContextCompat.RECEIVER_NOT_EXPORTED)
        try {
            val sent = ArrayList(parts.indices.map {
                PendingIntent.getBroadcast(
                    context, it, Intent(action).setPackage(context.packageName),
                    PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_ONE_SHOT,
                )
            })
            sms.sendMultipartTextMessage(to, null, parts, sent, null)
            if (!done.await(SEND_TIMEOUT_S, TimeUnit.SECONDS)) return "unknown"
        } finally {
            context.unregisterReceiver(receiver)
        }
        return synchronized(codes) {
            when {
                codes.all { it == Activity.RESULT_OK } -> "sent"
                codes.any { it == Activity.RESULT_OK } -> "unknown"
                codes.any { it == SmsManager.RESULT_ERROR_NO_SERVICE || it == SmsManager.RESULT_ERROR_RADIO_OFF } -> "no_signal"
                else -> "failed"
            }
        }
    }

    /** The contact name of [number]; null without one or without contacts permission. */
    fun contactName(context: Context, number: String): String? {
        if (number.isEmpty() ||
            ContextCompat.checkSelfPermission(context, Manifest.permission.READ_CONTACTS) != PackageManager.PERMISSION_GRANTED
        ) {
            return null
        }
        val uri = Uri.withAppendedPath(ContactsContract.PhoneLookup.CONTENT_FILTER_URI, Uri.encode(number))
        return context.contentResolver.query(uri, arrayOf(ContactsContract.PhoneLookup.DISPLAY_NAME), null, null, null)
            ?.use { if (it.moveToFirst()) it.getString(0) else null }
    }
}

/** `SMS_RECEIVED`: each incoming SMS goes to [Events] while a watch runs. */
class SmsReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        if (intent.action != Telephony.Sms.Intents.SMS_RECEIVED_ACTION || !Events.watching) return
        val parts = Telephony.Sms.Intents.getMessagesFromIntent(intent)
        val first = parts.firstOrNull() ?: return
        val address = first.originatingAddress ?: ""
        // The default SMS app stores it (and gives it an id) later: id 0 is
        // provisional (`phone::SmsMessage::id`); its thread is known now.
        val thread = try {
            Telephony.Threads.getOrCreateThreadId(context, address)
        } catch (e: Exception) {
            0L
        }
        Events.offer(
            JSONObject()
                .put("type", "sms_received")
                .put("id", 0)
                .put("thread_id", thread)
                .put("outgoing", false)
                .put("text", parts.joinToString("") { it.messageBody ?: "" })
                // Receive time, as the provider's `date` will be (not the sender's clock).
                .put("when_ms", System.currentTimeMillis())
                .toString(),
        )
    }
}

/** What [Debounce] needs of a looper: run later, or not. */
interface Timer {
    fun post(task: Runnable, delayMs: Long)
    fun cancel(task: Runnable)
}

class HandlerTimer(private val handler: Handler) : Timer {
    override fun post(task: Runnable, delayMs: Long) {
        handler.postDelayed(task, delayMs)
    }

    override fun cancel(task: Runnable) = handler.removeCallbacks(task)
}

/** Runs [action] once, [delayMs] after the last [poke]: a burst is one run. */
class Debounce(private val delayMs: Long, private val timer: Timer, private val action: () -> Unit) {
    private val task = Runnable { action() }

    fun poke() {
        timer.cancel(task)
        timer.post(task, delayMs)
    }
}
