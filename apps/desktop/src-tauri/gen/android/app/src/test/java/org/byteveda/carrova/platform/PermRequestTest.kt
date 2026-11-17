package org.byteveda.carrova.platform

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Test

class PermRequestTest {
    @Test
    fun a_second_request_is_refused_until_the_first_ends() {
        val first = PermRequest.begin()
        assertNotNull(first)
        assertNull(PermRequest.begin())

        PermRequest.done()
        assertEquals(0L, first!!.count)
        PermRequest.end(first)

        val next = PermRequest.begin()
        assertNotNull(next)
        PermRequest.end(next!!)
    }
}
