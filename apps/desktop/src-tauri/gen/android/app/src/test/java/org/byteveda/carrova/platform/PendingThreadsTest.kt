package org.byteveda.carrova.platform

import org.junit.Assert.assertEquals
import org.junit.Test

class PendingThreadsTest {
    private var now = 1_000L
    private val pending = PendingThreads { now }

    @Test
    fun resolves_only_stored_threads_once() {
        pending.add(7)
        assertEquals(emptyList<Long>(), pending.resolve { _, _ -> false })
        // Still pending: asked with its receive time.
        assertEquals(listOf(7L), pending.resolve { t, since -> t == 7L && since == 1_000L - 5_000L })
        assertEquals(emptyList<Long>(), pending.resolve { _, _ -> true })
    }

    @Test
    fun two_threads_resolve_in_one_pass() {
        pending.add(1)
        pending.add(2)
        assertEquals(setOf(1L, 2L), pending.resolve { _, _ -> true }.toSet())
    }

    @Test
    fun expires_after_ten_seconds() {
        pending.add(7)
        now += 10_000
        assertEquals(listOf(7L), pending.resolve { _, _ -> true }.also { pending.add(7) })
        pending.clear()
        pending.add(8)
        now += 10_001
        assertEquals(emptyList<Long>(), pending.resolve { _, _ -> true })
    }

    @Test
    fun row_stored_2s_before_receipt_resolves() {
        pending.add(7)
        val rowDate = now - 2_000
        assertEquals(listOf(7L), pending.resolve { _, since -> rowDate >= since })
    }

    @Test
    fun clear_forgets() {
        pending.add(7)
        pending.clear()
        assertEquals(emptyList<Long>(), pending.resolve { _, _ -> true })
    }
}
