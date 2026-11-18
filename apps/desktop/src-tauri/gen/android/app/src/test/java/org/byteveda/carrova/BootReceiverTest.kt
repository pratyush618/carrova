package org.byteveda.carrova

import android.content.Intent
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class BootReceiverTest {
    @Test
    fun boot_starts_the_link() {
        assertTrue(BootReceiver.startsLink(Intent.ACTION_BOOT_COMPLETED))
    }

    /** An update (`adb install -r`, Play) force-stops the app and drops the sticky service. */
    @Test
    fun an_app_update_starts_the_link_again() {
        assertTrue(BootReceiver.startsLink(Intent.ACTION_MY_PACKAGE_REPLACED))
    }

    @Test
    fun other_broadcasts_do_not() {
        assertFalse(BootReceiver.startsLink(Intent.ACTION_PACKAGE_REPLACED))
        assertFalse(BootReceiver.startsLink(null))
    }
}
