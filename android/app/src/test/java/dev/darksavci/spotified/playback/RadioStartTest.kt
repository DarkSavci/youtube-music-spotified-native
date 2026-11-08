package dev.darksavci.spotified.playback

import dev.darksavci.spotified.api.PlayState
import dev.darksavci.spotified.api.Projection
import dev.darksavci.spotified.api.Queue
import dev.darksavci.spotified.api.SessionState
import dev.darksavci.spotified.api.Target
import dev.darksavci.spotified.api.Track
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class RadioStartTest {
    private val song = Track(id = "s", title = "s")

    @Test
    fun aSongFromTheResultsStartsItsRadio() {
        assertEquals(RadioStart.Radio, radioStart(Projection(), song))
    }

    @Test
    fun theSongAlreadyPlayingIsNotStartedOver() {
        val queue = SessionState(queue = Queue(listOf(song), 0))
        assertEquals(RadioStart.Nothing, radioStart(Projection(state = queue, target = Target(playing = true)), song))
        assertEquals(RadioStart.Resume, radioStart(Projection(state = queue, target = Target(playing = false)), song))
    }

    @Test
    fun inARoomTheRoomDecidesAndAnUnplayableSongDoesNothing() {
        assertEquals(RadioStart.Room, radioStart(Projection(followingRoom = true), song))
        assertEquals(RadioStart.Nothing, radioStart(Projection(), song.copy(playable = false)))
    }
}
