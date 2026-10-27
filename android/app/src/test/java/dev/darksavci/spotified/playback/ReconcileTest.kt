package dev.darksavci.spotified.playback

import dev.darksavci.spotified.api.Target
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class ReconcileTest {
    private fun target(epoch: Long, id: String, at: Long = 0, next: String = "") =
        Target(epoch = epoch, videoId = id, startAtMs = at, playing = true, preloadVideoId = next)

    @Test
    fun nothingToPlayStopsBothDecks() {
        assertEquals(listOf(Op.Stop), reconcile(Decks(current = "a"), target(1, "a"), Target(epoch = 2)))
    }

    @Test
    fun aNewTrackLoadsFromWhereTheCoreSays() {
        assertEquals(listOf(Op.Load(42_000)), reconcile(Decks(), Target(), target(1, "a", at = 42_000)))
    }

    @Test
    fun theTrackOnThePreloadDeckIsPromotedNotLoaded() {
        val ops = reconcile(Decks(current = "a", next = "b"), target(1, "a", next = "b"), target(2, "b"))
        assertEquals(listOf(Op.Promote), ops)
    }

    @Test
    fun aPromotedTrackStartingLaterIsSought() {
        val ops = reconcile(Decks(current = "a", next = "b"), target(1, "a"), target(2, "b", at = 30_000))
        assertEquals(listOf(Op.Promote, Op.Seek(30_000)), ops)
    }

    @Test
    fun driftBetweenClocksIsNotASeek() {
        val ops = reconcile(Decks(current = "a", positionMs = 10_900), target(1, "a", at = 10_000), target(1, "a", at = 10_000))
        assertTrue(ops.isEmpty())
    }

    @Test
    fun aRealSeekIsFollowed() {
        val ops = reconcile(Decks(current = "a", positionMs = 10_000), target(1, "a", at = 10_000), target(1, "a", at = 95_000))
        assertEquals(listOf(Op.Seek(95_000)), ops)
    }

    @Test
    fun repeatOneRestartsAnEndedDeck() {
        val ops = reconcile(Decks(current = "a", positionMs = 200_000, ended = true), target(1, "a"), target(2, "a", at = 0))
        assertEquals(listOf(Op.Seek(0)), ops)
    }

    @Test
    fun aTrackTheDeckRanIntoIsLeftPlaying() {
        // The engine went on to "b" by itself; the core names it a moment later.
        val ops = reconcile(Decks(current = "b", positionMs = 4_000), target(1, "a", next = "b"), target(2, "b", at = 0))
        assertTrue(ops.isEmpty())
    }

    @Test
    fun theNextTrackIsPreloadedAndAStaleOneDropped() {
        assertEquals(listOf(Op.Preload), reconcile(Decks(current = "a"), target(1, "a"), target(1, "a", next = "b")))
        assertEquals(listOf(Op.Preload), reconcile(Decks(current = "a", next = "x"), target(1, "a"), target(1, "a", next = "b")))
        assertEquals(listOf(Op.DropPreload), reconcile(Decks(current = "a", next = "b"), target(1, "a", next = "b"), target(1, "a")))
        assertTrue(reconcile(Decks(current = "a", next = "b"), target(1, "a", next = "b"), target(1, "a", next = "b")).isEmpty())
    }

    @Test
    fun aFadeStartsItsLengthBeforeTheEnd() {
        assertFalse(crossfadeDue(170_000, 180_000, 5_000, 1f))
        assertTrue(crossfadeDue(175_000, 180_000, 5_000, 1f))
        assertFalse(crossfadeDue(5_000, 8_000, 5_000, 1f))
        assertFalse(crossfadeDue(175_000, 180_000, 0, 1f))
    }
}
