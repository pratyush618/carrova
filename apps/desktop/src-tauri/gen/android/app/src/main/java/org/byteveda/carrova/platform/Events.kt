package org.byteveda.carrova.platform

/**
 * Kotlin -> Rust events (spec "Android bridge"): JSON strings pulled by the
 * one Rust thread of `watch_phone_events` through [take]. Bounded: past
 * [CAP] the oldest go and one `dropped` event (coalesced while queued)
 * tells Rust how many, so it resends its snapshot. Closed until [open].
 */
object Events {
    const val CAP = 256

    /** Stands in the queue for the events lost since it was queued. */
    private class Dropped(var count: Int)

    private val lock = Object()
    private val queue = ArrayDeque<Any>()
    private var dropped: Dropped? = null
    private var closed = true

    /** Whether a watch takes events; senders skip building them otherwise. */
    @JvmStatic
    val watching: Boolean
        get() = synchronized(lock) { !closed }

    /** Start a watch: an empty, open queue. */
    @JvmStatic
    fun open() {
        synchronized(lock) {
            queue.clear()
            dropped = null
            closed = false
        }
    }

    /** Queue [json]; ignored while closed. */
    @JvmStatic
    fun offer(json: String) {
        synchronized(lock) {
            if (closed) return
            while (queue.size >= CAP) dropOldest()
            queue.addLast(json)
            lock.notifyAll()
        }
    }

    /** Block for the next event; null once [close] was called. */
    @JvmStatic
    fun take(): String? {
        synchronized(lock) {
            while (queue.isEmpty() && !closed) lock.wait()
            if (closed) return null
            val next = queue.removeFirst()
            if (next === dropped) {
                dropped = null
                return "{\"type\":\"dropped\",\"count\":${(next as Dropped).count}}"
            }
            return next as String
        }
    }

    /** End the watch: drop what is queued and unblock [take] for good. */
    @JvmStatic
    fun close() {
        synchronized(lock) {
            closed = true
            queue.clear()
            dropped = null
            lock.notifyAll()
        }
    }

    private fun dropOldest() {
        // The marker is not an event; at most one is queued, so an event is.
        queue.removeAt(queue.indexOfFirst { it is String })
        val marker = dropped ?: Dropped(0).also {
            dropped = it
            queue.addLast(it)
        }
        marker.count++
    }
}
