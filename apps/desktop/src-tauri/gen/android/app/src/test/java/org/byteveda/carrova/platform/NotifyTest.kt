package org.byteveda.carrova.platform

import org.junit.Assert.assertEquals
import org.junit.Test

class NotifyTest {
    @Test
    fun app_label_wins_over_the_package_id() {
        assertEquals("Discord", Notify.appName(null, "Discord", "com.discord"))
    }

    @Test
    fun a_system_stand_in_name_wins_over_the_label() {
        assertEquals("Android System", Notify.appName("Android System", "System UI", "android"))
    }

    @Test
    fun blank_names_fall_back_to_the_package_id_last() {
        assertEquals("Discord", Notify.appName("  ", " Discord ", "com.discord"))
        assertEquals("com.discord", Notify.appName("", null, "com.discord"))
    }
}
