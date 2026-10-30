package dev.darksavci.spotified.data

import dev.darksavci.spotified.api.RemoteQueue
import dev.darksavci.spotified.api.Track

/** What of the account's queue on another device can be played here. A port of the desktop's `pick_remote`. */
object RemotePickup {
    data class Found(val tracks: List<Track>, val index: Int, val origin: String)

    /** The playable songs, starting at the one that device was on or the first playable after it. Null when none is left. */
    fun pick(queue: RemoteQueue): Found? {
        val current = queue.index.coerceAtMost((queue.tracks.size - 1).coerceAtLeast(0))
        var start: Int? = null
        val tracks = mutableListOf<Track>()
        queue.tracks.forEachIndexed { i, t ->
            if (!t.playable) return@forEachIndexed
            if (start == null && i >= current) start = tracks.size
            tracks += t
        }
        if (tracks.isEmpty()) return null
        return Found(tracks, start ?: tracks.lastIndex, queue.title.trim().ifEmpty { "YouTube Music" })
    }
}
