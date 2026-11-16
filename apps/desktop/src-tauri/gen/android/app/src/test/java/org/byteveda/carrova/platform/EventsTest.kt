package org.byteveda.carrova.platform

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test

class EventsTest {
    private var gen = 0L

    @Before
    fun reset() {
        gen = Events.open()
    }

    private fun takeAll(n: Int): List<String> = List(n) { Events.take(gen)!! }

    @Test
    fun queue_drops_oldest_past_cap() {
        repeat(Events.CAP + 10) { Events.offer("e$it") }
        val got = takeAll(Events.CAP)
        // Oldest ones went; the newest is last.
        assertFalse(got.contains("e0"))
        assertEquals("e${Events.CAP + 9}", got.last())
    }

    @Test
    fun overflow_enqueues_one_dropped_event() {
        repeat(Events.CAP + 10) { Events.offer("e$it") }
        val got = takeAll(Events.CAP)
        val dropped = got.filter { it.startsWith("{\"type\":\"dropped\"") }
        assertEquals(1, dropped.size)
        val kept = got.count { it.startsWith("e") }
        assertEquals("{\"type\":\"dropped\",\"count\":${Events.CAP + 10 - kept}}", dropped[0])
        // Taken, so the next overflow starts a new one.
        repeat(Events.CAP + 1) { Events.offer("f$it") }
        assertEquals(1, takeAll(Events.CAP).count { it.contains("dropped") })
    }

    @Test
    fun close_unblocks_take() {
        var got: String? = "unset"
        val taker = Thread { got = Events.take(gen) }
        taker.start()
        Thread.sleep(100)
        Events.close(gen)
        taker.join(2000)
        assertFalse(taker.isAlive)
        assertNull(got)
        assertNull(Events.take(gen))
    }

    @Test
    fun open_after_close_resets() {
        Events.offer("old")
        Events.close(gen)
        Events.offer("while closed")
        gen = Events.open()
        Events.offer("new")
        assertEquals("new", Events.take(gen))
    }

    @Test
    fun stale_close_keeps_new_watch() {
        var got: String? = "unset"
        val oldTaker = Thread { got = Events.take(gen) }
        oldTaker.start()
        Thread.sleep(100)
        val newer = Events.open()
        // The replaced watch's taker ends, so its guard's join returns.
        oldTaker.join(2000)
        assertFalse(oldTaker.isAlive)
        assertNull(got)
        Events.close(gen) // the replaced watch's guard drops late
        assertTrue(Events.watching)
        Events.offer("kept")
        assertEquals("kept", Events.take(newer))
        Events.close(newer)
        assertFalse(Events.watching)
    }
}
