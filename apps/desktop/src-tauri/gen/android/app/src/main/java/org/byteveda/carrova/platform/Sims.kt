package org.byteveda.carrova.platform

import android.Manifest
import android.content.Context
import android.content.pm.PackageManager
import android.os.Build
import android.telecom.PhoneAccountHandle
import android.telecom.TelecomManager
import android.telephony.SmsManager
import android.telephony.SubscriptionManager
import android.telephony.TelephonyManager
import androidx.core.content.ContextCompat
import org.json.JSONArray
import org.json.JSONObject

/**
 * Dual-SIM glue for `carrova-platform` (android/phone.rs): the active SIMs,
 * and the [SmsManager] and phone account of the one a text or call goes
 * from. A subscription id of -1 (Android's invalid id) is the phone's
 * default. The choice needs Android 12 (`createForSubscriptionId`); before
 * it, or without `READ_PHONE_STATE`, no SIMs are listed and the default is used.
 */
object Sims {
    /** Longest SIM name sent (the link refuses longer). */
    private const val MAX_NAME = 64

    /** The active SIMs as `phone::Sim` JSON, by slot; `[]` when they may not be read. */
    @JvmStatic
    fun list(context: Context): String {
        val sims = JSONArray()
        if (Build.VERSION.SDK_INT < 31 || !canRead(context)) return sims.toString()
        val subs = try {
            context.getSystemService(SubscriptionManager::class.java).activeSubscriptionInfoList
        } catch (e: SecurityException) {
            null
        } ?: return sims.toString()
        val sms = SubscriptionManager.getDefaultSmsSubscriptionId()
        val voice = SubscriptionManager.getDefaultVoiceSubscriptionId()
        // A slot of -1 is no SIM card in this phone (e.g. a remote SIM): not offered.
        for (sub in subs.filter { it.simSlotIndex >= 0 }.sortedBy { it.simSlotIndex }) {
            val name = sub.displayName?.toString().orEmpty().ifEmpty { sub.carrierName?.toString().orEmpty() }
            sims.put(
                JSONObject()
                    .put("id", sub.subscriptionId)
                    .put("slot", sub.simSlotIndex)
                    .put("name", name.take(MAX_NAME))
                    .put("sms_default", sub.subscriptionId == sms)
                    .put("voice_default", sub.subscriptionId == voice),
            )
        }
        return sims.toString()
    }

    /** The [SmsManager] that sends from subscription [sub] (-1: the default). */
    fun smsManager(context: Context, sub: Int): SmsManager = if (Build.VERSION.SDK_INT >= 31) {
        val sms = context.getSystemService(SmsManager::class.java)
        if (sub >= 0) sms.createForSubscriptionId(sub) else sms
    } else {
        @Suppress("DEPRECATION")
        SmsManager.getDefault()
    }

    /**
     * The phone account that calls from subscription [sub]; null for the default (-1).
     * Throws when no account matches, so the call fails rather than go from another SIM.
     */
    fun account(context: Context, sub: Int): PhoneAccountHandle? {
        if (sub < 0 || Build.VERSION.SDK_INT < 31) return null
        val phone = context.getSystemService(TelephonyManager::class.java)
        val accounts = try {
            context.getSystemService(TelecomManager::class.java).callCapablePhoneAccounts
        } catch (e: SecurityException) {
            emptyList()
        }
        return pick(sub, accounts) { phone.getSubscriptionId(it) }
            ?: throw IllegalStateException("no phone account for that SIM")
    }

    /** The first of [accounts] whose subscription ([subOf]) is [sub]. */
    internal fun <T> pick(sub: Int, accounts: List<T>, subOf: (T) -> Int): T? =
        accounts.firstOrNull { subOf(it) == sub }

    /** Phone and SIM state may be read. */
    private fun canRead(context: Context) =
        ContextCompat.checkSelfPermission(context, Manifest.permission.READ_PHONE_STATE) ==
            PackageManager.PERMISSION_GRANTED

    /** Logical modems active (2 on a dual-SIM phone, even with one SIM in); 1 before Android 12. */
    fun modems(context: Context): Int =
        if (Build.VERSION.SDK_INT >= 31) context.getSystemService(TelephonyManager::class.java).activeModemCount else 1
}
