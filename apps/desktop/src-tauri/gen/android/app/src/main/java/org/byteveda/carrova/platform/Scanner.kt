package org.byteveda.carrova.platform

import android.content.Context
import android.content.Intent
import java.util.concurrent.CompletableFuture
import java.util.concurrent.TimeUnit
import java.util.concurrent.TimeoutException

/**
 * QR scan glue for `carrova-platform` (android/scan.rs): opens [ScanActivity] (ZXing, offline)
 * and waits for its text. Rust checks what the text is.
 */
object Scanner {
    /** A forgotten camera screen gives up after this long. */
    private const val TIMEOUT_MINUTES = 5L

    @Volatile private var pending: CompletableFuture<String?>? = null

    /** The scanned text; null if cancelled or timed out. Blocks (called off the main thread). */
    @JvmStatic
    fun scan(context: Context): String? {
        val result = CompletableFuture<String?>()
        pending?.complete(null) // a newer scan replaces an unanswered one
        pending = result
        context.startActivity(Intent(context, ScanActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
        return try {
            result.get(TIMEOUT_MINUTES, TimeUnit.MINUTES)
        } catch (e: TimeoutException) {
            null
        } finally {
            if (pending === result) pending = null
        }
    }

    internal fun deliver(text: String?) {
        pending?.complete(text)
    }
}
