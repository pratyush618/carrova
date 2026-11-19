package org.byteveda.carrova.platform

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class UpdatesTest {
    @Test
    fun a_digest_is_32_unsigned_byte_values() {
        // SHA-256("abc") starts ba 78 16 bf.
        val digest = Updates.sha256("abc".toByteArray())
        assertEquals(32, digest.size)
        assertEquals(listOf(0xba, 0x78, 0x16, 0xbf), digest.take(4))
    }

    @Test
    fun a_tap_is_taken_once() {
        UpdateActions.tap(UpdateActions.PAUSE)
        assertEquals("pause", Updates.takeTap())
        assertNull(Updates.takeTap())
    }

    @Test
    fun unknown_actions_are_not_taps() {
        UpdateActions.tap("something.else")
        assertNull(Updates.takeTap())
    }
}
