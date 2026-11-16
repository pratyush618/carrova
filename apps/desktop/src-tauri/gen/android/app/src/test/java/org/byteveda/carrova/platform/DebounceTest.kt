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
}
