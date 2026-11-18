package org.byteveda.carrova.platform

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.result.contract.ActivityResultContracts
import java.util.concurrent.CountDownLatch

/**
 * Shows Android's "Allow Carrova to …?" dialog for [Perms.request] and finishes on the answer.
 * It exists because the glue gets a Context, not the running activity the dialog needs.
 */
class PermRequestActivity : ComponentActivity() {
    private val ask = registerForActivityResult(ActivityResultContracts.RequestMultiplePermissions()) {
        finish()
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val perms = intent.getStringArrayExtra(PERMS)
        if (perms == null) {
            finish()
        } else if (savedInstanceState == null) {
            ask.launch(perms)
        }
    }

    override fun onDestroy() {
        super.onDestroy()
        // Answered, or closed without one; a rotation recreates it and the answer comes later.
        if (!isChangingConfigurations) PermRequest.done()
    }

    internal companion object {
        const val PERMS = "perms"
    }
}

/** Hands one permission request between [Perms.request] and [PermRequestActivity]. */
internal object PermRequest {
    private var pending: CountDownLatch? = null

    /** A latch the activity opens when done; null while another request is open. */
    @Synchronized
    fun begin(): CountDownLatch? = if (pending != null) null else CountDownLatch(1).also { pending = it }

    /** The open request is answered, or its screen is gone. */
    @Synchronized
    fun done() {
        pending?.countDown()
    }

    /** Forget [latch], so the next request may start. */
    @Synchronized
    fun end(latch: CountDownLatch) {
        if (pending === latch) pending = null
    }
}
