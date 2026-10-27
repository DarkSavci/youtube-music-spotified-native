package dev.darksavci.spotified.playback

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.drop
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.launch

/** Stops the music after a while, or when the song playing ends. */
class SleepTimer(private val scope: CoroutineScope, private val playback: Playback) {
    private val _endsAt = MutableStateFlow<Long?>(null)
    /** When it stops, as wall-clock milliseconds; -1 for "at the end of this song"; null when off. */
    val endsAt: StateFlow<Long?> = _endsAt
    private var job: Job? = null

    fun after(minutes: Int) {
        cancel()
        val at = System.currentTimeMillis() + minutes * 60_000L
        _endsAt.value = at
        job = scope.launch {
            delay(at - System.currentTimeMillis())
            stop()
        }
    }

    fun atEndOfSong() {
        cancel()
        _endsAt.value = END_OF_SONG
        job = scope.launch {
            // The next song starting is the end of this one.
            playback.projection.map { it.state.current?.id }.distinctUntilChanged().drop(1).first()
            stop()
        }
    }

    fun cancel() {
        job?.cancel()
        job = null
        _endsAt.value = null
    }

    private fun stop() {
        playback.pause()
        _endsAt.value = null
        job = null
    }

    companion object {
        const val END_OF_SONG = -1L
    }
}
