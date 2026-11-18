package org.byteveda.carrova.platform

import org.junit.Assert.assertEquals
import org.junit.Test

class PermsTest {
    @Test
    fun hidden_checks_read_as_denied() {
        assertEquals("denied", Perms.stateOf({ false }, { throw SecurityException("hidden op") }))
        assertEquals("denied", Perms.stateOf({ throw SecurityException("hidden") }, { false }))
    }

    @Test
    fun unknown_restricted_op_reads_as_denied() {
        assertEquals("denied", Perms.stateOf({ false }, { throw IllegalArgumentException("unknown op") }))
    }

    @Test
    fun answers_decide_otherwise() {
        assertEquals("granted", Perms.stateOf({ true }, { true }))
        assertEquals("restricted", Perms.stateOf({ false }, { true }))
        assertEquals("denied", Perms.stateOf({ false }, { false }))
    }
}
