package org.byteveda.carrova.platform

import org.junit.Assert.assertEquals
import org.junit.Test

class DebounceTest {
    /** A clock that runs due tasks as time advances. */
    private class FakeTimer : Timer {
        var now = 0L
        private var pending: Pair<Runnable, Long>? = null

        override fun post(task: Runnable, delayMs: Long) {
            pending = task to now + delayMs
        }

        override fun cancel(task: Runnable) {
            if (pending?.first === task) pending = null
        }

        fun advance(ms: Long) {
            now += ms
            pending?.takeIf { it.second <= now }?.let {
                pending = null
                it.first.run()
            }
        }
    }

    @Test
    fun burst_is_one_emit_after_the_last_change() {
        val timer = FakeTimer()
        var emits = 0
        val d = Debounce(500, timer) { emits++ }
        d.poke()
        timer.advance(400)
        d.poke()
        timer.advance(400)
        assertEquals(0, emits)
        timer.advance(100)
        assertEquals(1, emits)
    }

    @Test
    fun changes_600ms_apart_emit_twice() {
        val timer = FakeTimer()
        var emits = 0
        val d = Debounce(500, timer) { emits++ }
        d.poke()
        timer.advance(600)
        d.poke()
        timer.advance(600)
        assertEquals(2, emits)
    }

    // Debounce + PendingThreads composed as `Sms` does; not the `expectStored` wiring.
    @Test
    fun receipt_of_an_already_stored_row_emits_once_with_no_provider_change() {
        val timer = FakeTimer()
        val pending = PendingThreads { timer.now }
        val emitted = mutableListOf<Long>()
        val d = Debounce(500, timer) { emitted += pending.resolve { _, _ -> true } }
        // What `expectStored` does: remember the thread, then poke.
        pending.add(7)
        d.poke()
        timer.advance(499)
        assertEquals(0, emitted.size)
        timer.advance(1)
        assertEquals(listOf(7L), emitted)
        timer.advance(5_000)
        assertEquals(listOf(7L), emitted)
    }
}
