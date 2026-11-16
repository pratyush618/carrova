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

    /** The current watch's number; a guard closes only its own watch. */
    @JvmStatic
    var generation = 0L
        get() = synchronized(lock) { field }
        private set

    /** Whether a watch takes events; senders skip building them otherwise. */
    @JvmStatic
    val watching: Boolean
        get() = synchronized(lock) { !closed }

    /** Start a watch (replacing any): an empty, open queue; returns its generation. */
    @JvmStatic
    fun open(): Long {
        synchronized(lock) {
            queue.clear()
            dropped = null
            closed = false
            generation++
            lock.notifyAll() // a replaced watch's taker ends
            return generation
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

    /** Block for the next event of watch [gen]; null once it is closed or replaced. */
    @JvmStatic
    fun take(gen: Long): String? {
        synchronized(lock) {
            while (gen == generation && queue.isEmpty() && !closed) lock.wait()
            if (gen != generation || closed) return null
            val next = queue.removeFirst()
            if (next === dropped) {
                dropped = null
                return "{\"type\":\"dropped\",\"count\":${(next as Dropped).count}}"
            }
            return next as String
        }
    }

    /** End watch [gen]: drop what is queued and unblock [take]. A replaced watch's [gen] does nothing. */
    @JvmStatic
    fun close(gen: Long) {
        synchronized(lock) {
            if (gen != generation) return
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
