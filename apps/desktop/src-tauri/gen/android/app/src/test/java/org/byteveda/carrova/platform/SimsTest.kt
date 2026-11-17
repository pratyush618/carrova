package org.byteveda.carrova.platform

import android.Manifest
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class SimsTest {
    @Test
    fun picks_the_account_of_the_chosen_sim() {
        val accounts = listOf("slot1" to 3, "slot2" to 7)
        assertEquals("slot2" to 7, Sims.pick(7, accounts) { it.second })
        assertNull(Sims.pick(9, accounts) { it.second })
    }

    @Test
    fun texts_ask_for_phone_state_only_with_more_than_one_modem() {
        val sms = arrayOf(Manifest.permission.READ_SMS, Manifest.permission.SEND_SMS, Manifest.permission.RECEIVE_SMS)
        assertArrayEquals(sms, Perms.asked("sms", 1))
        assertArrayEquals(sms + Manifest.permission.READ_PHONE_STATE, Perms.asked("sms", 2))
        assertArrayEquals(arrayOf(Manifest.permission.READ_CONTACTS), Perms.asked("contacts", 2))
    }
}
