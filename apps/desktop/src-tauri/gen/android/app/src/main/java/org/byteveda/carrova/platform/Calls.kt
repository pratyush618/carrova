@file:Suppress("DEPRECATION") // PhoneStateListener, acceptRingingCall, endCall: see S2

package org.byteveda.carrova.platform

import android.annotation.SuppressLint
import android.content.Context
import android.media.AudioDeviceInfo
import android.media.AudioManager
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.provider.CallLog
import android.telecom.TelecomManager
import android.telephony.PhoneStateListener
import android.telephony.TelephonyManager
import org.json.JSONArray
import org.json.JSONObject

/**
 * Call glue for `carrova-platform` (android/phone.rs): the call state as
 * [Events], call controls through [TelecomManager] and [AudioManager], the
 * call log. Rust decides when to use which (e.g. speaker on answer).
 */
@SuppressLint("MissingPermission") // Rust checks `phone_perm` first; a refusal is an error there.
object Calls {
    private var listener: StateListener? = null

    /** Report call state changes to [Events] ([on]) or stop. */
    @JvmStatic
    @Synchronized
    fun listen(context: Context, on: Boolean) {
        val phone = context.getSystemService(TelephonyManager::class.java)
        listener?.let {
            phone.listen(it, PhoneStateListener.LISTEN_NONE)
        }
        listener = null
        if (!on) return
        val next = StateListener(context.applicationContext)
        // PhoneStateListener, not TelephonyCallback: only it carries the ringing number.
        phone.listen(next, PhoneStateListener.LISTEN_CALL_STATE)
        listener = next
    }

    @JvmStatic
    fun answer(context: Context) {
        telecom(context).acceptRingingCall()
    }

    @JvmStatic
    fun decline(context: Context) = end(context)

    @JvmStatic
    fun end(context: Context) {
        if (!telecom(context).endCall()) throw IllegalStateException("endCall refused")
    }

    @JvmStatic
    fun mute(context: Context, on: Boolean) {
        audio(context).isMicrophoneMute = on
    }

    @JvmStatic
    fun dial(context: Context, number: String) {
        telecom(context).placeCall(Uri.fromParts("tel", number, null), Bundle())
    }

    @JvmStatic
    fun speaker(context: Context, on: Boolean) {
        val audio = audio(context)
        if (Build.VERSION.SDK_INT >= 31) {
            val speaker = audio.availableCommunicationDevices
                .firstOrNull { it.type == AudioDeviceInfo.TYPE_BUILTIN_SPEAKER }
            if (on && speaker != null) audio.setCommunicationDevice(speaker) else audio.clearCommunicationDevice()
        } else {
            audio.isSpeakerphoneOn = on
        }
    }

    /** Up to [limit] call-log entries before the `(beforeMs, beforeId)` pair, newest first. */
    @JvmStatic
    fun history(context: Context, beforeMs: Long, beforeId: Long, limit: Int): String {
        val records = JSONArray()
        val projection = arrayOf(
            CallLog.Calls._ID, CallLog.Calls.NUMBER, CallLog.Calls.CACHED_NAME,
            CallLog.Calls.DATE, CallLog.Calls.DURATION, CallLog.Calls.TYPE,
        )
        context.contentResolver.query(
            CallLog.Calls.CONTENT_URI, projection,
            "${CallLog.Calls.DATE} < ? OR (${CallLog.Calls.DATE} = ? AND ${CallLog.Calls._ID} < ?)",
            arrayOf(beforeMs.toString(), beforeMs.toString(), beforeId.toString()),
            "${CallLog.Calls.DATE} DESC, ${CallLog.Calls._ID} DESC",
        )?.use { c ->
            while (records.length() < limit && c.moveToNext()) {
                val kind = when (c.getInt(5)) {
                    CallLog.Calls.INCOMING_TYPE -> "incoming"
                    CallLog.Calls.OUTGOING_TYPE -> "outgoing"
                    CallLog.Calls.MISSED_TYPE, CallLog.Calls.VOICEMAIL_TYPE -> "missed"
                    CallLog.Calls.REJECTED_TYPE, CallLog.Calls.BLOCKED_TYPE -> "declined"
                    else -> continue
                }
                records.put(
                    JSONObject()
                        .put("id", c.getLong(0))
                        .put("number", c.getString(1) ?: "")
                        .put("name", c.getString(2) ?: JSONObject.NULL)
                        .put("when_ms", c.getLong(3))
                        .put("duration_s", c.getLong(4))
                        .put("kind", kind),
                )
            }
        }
        return records.toString()
    }

    private fun telecom(context: Context) = context.getSystemService(TelecomManager::class.java)

    private fun audio(context: Context) = context.getSystemService(AudioManager::class.java)

    /** Android's three call states as `phone::CallState` events. */
    private class StateListener(private val context: Context) : PhoneStateListener(context.mainExecutor) {
        private var ringing = false
        private var answered = false

        @Deprecated("PhoneStateListener")
        override fun onCallStateChanged(state: Int, number: String?) {
            val event = JSONObject()
            when (state) {
                TelephonyManager.CALL_STATE_RINGING -> {
                    ringing = true
                    answered = false
                    val caller = number ?: ""
                    event.put("type", "call_ringing").put("number", caller)
                        .put("name", Sms.contactName(context, caller) ?: JSONObject.NULL)
                }
                TelephonyManager.CALL_STATE_OFFHOOK -> {
                    answered = true
                    event.put("type", "call_active").put("since_ms", System.currentTimeMillis())
                        .put("muted", audio(context).isMicrophoneMute)
                }
                else -> {
                    // The first report on listening is the idle phone: nothing ended.
                    if (!ringing && !answered) return
                    event.put("type", "call_ended").put("missed", ringing && !answered)
                    ringing = false
                    answered = false
                }
            }
            Events.offer(event.toString())
        }
    }
}
