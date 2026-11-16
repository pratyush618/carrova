package org.byteveda.carrova.platform

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class SmsStoredGateTest {
    @Test
    fun one_event_per_thread_per_window() {
        var now = 1_000L
        val gate = StoredGate { now }
        assertTrue(gate.allow(7))
        now += 499
        assertFalse(gate.allow(7))
        assertTrue(gate.allow(8))
        now += 1
        assertTrue(gate.allow(7))
    }
}
