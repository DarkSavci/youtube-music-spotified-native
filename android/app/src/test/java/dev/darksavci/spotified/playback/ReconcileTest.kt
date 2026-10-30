package dev.darksavci.spotified.playback

import dev.darksavci.spotified.api.CoreJson
import dev.darksavci.spotified.api.Target
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class ReconcileTest {
    private fun target(epoch: Long, id: String, at: Long = 0, next: String = "", seq: Long = 0) =
        Target(epoch = epoch, videoId = id, startAtMs = at, seekSeq = seq, playing = true, preloadVideoId = next)

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
        assertEquals(listOf(Op.Promote, Op.Seek(30_000, SeekReason.Promoted)), ops)
    }

    @Test
    fun driftBetweenClocksIsNotASeek() {
        val ops = reconcile(Decks(current = "a", positionMs = 10_900), target(1, "a", at = 10_000), target(1, "a", at = 10_000, seq = 1))
        assertTrue(ops.isEmpty())
    }

    @Test
    fun aRealSeekIsFollowed() {
        val ops = reconcile(Decks(current = "a", positionMs = 10_000), target(1, "a", at = 10_000), target(1, "a", at = 95_000, seq = 1))
        assertEquals(listOf(Op.Seek(95_000, SeekReason.Asked)), ops)
    }

    @Test
    fun aSeekGoesWhereItNamedThoughALaterReportSaysOtherwise() {
        // The projection that carried the seek was dropped, and a report from
        // before it has since overwritten the position.
        val late = target(1, "a", at = 12_000, seq = 4).copy(seekToMs = 95_000)
        val ops = reconcile(Decks(current = "a", positionMs = 12_000), target(1, "a", at = 11_000, seq = 3), late)
        assertEquals(listOf(Op.Seek(95_000, SeekReason.Asked)), ops)
    }

    @Test
    fun aSeekToWhereTheDeckAlreadyIsMovesNothingWhateverTheEchoSays() {
        val late = target(1, "a", at = 12_000, seq = 4).copy(seekToMs = 95_000)
        assertTrue(reconcile(Decks(current = "a", positionMs = 95_400), target(1, "a", seq = 3), late).isEmpty())
    }

    @Test
    fun aDeckWithNothingLoadedStartsWhereTheSessionStands() {
        // The seek named is long past: the track has played on since.
        val going = target(1, "a", at = 150_000, seq = 4).copy(seekToMs = 30_000)
        assertEquals(listOf(Op.Load(150_000)), reconcile(Decks(), Target(), going))
    }

    @Test
    fun aRestartGoesWhereTheCoreNamed() {
        val again = target(2, "a", at = 700, seq = 5).copy(seekToMs = 0)
        val ops = reconcile(Decks(current = "a", positionMs = 200_000, ended = true), target(1, "a", seq = 4), again)
        assertEquals(listOf(Op.Seek(0, SeekReason.Restart)), ops)
    }

    @Test
    fun repeatOneRestartsAnEndedDeck() {
        val ops = reconcile(Decks(current = "a", positionMs = 200_000, ended = true), target(1, "a"), target(2, "a", at = 0))
        assertEquals(listOf(Op.Seek(0, SeekReason.Restart)), ops)
    }

    @Test
    fun aTrackTheDeckRanIntoIsLeftPlaying() {
        // The engine went on to "b" by itself; the core names it a moment later.
        val ops = reconcile(Decks(current = "b", positionMs = 4_000), target(1, "a", next = "b"), target(2, "b", at = 0, seq = 1))
        assertTrue(ops.isEmpty())
    }

    @Test
    fun aPositionEchoedBackIsNotASeek() {
        // After a crossfade: the deck is 6 s into "b", and the core publishes
        // the 0 it named the track with, as it does on the loaded report.
        val named = target(2, "b", at = 0, seq = 3)
        val deck = Decks(current = "b", positionMs = 6_000)
        assertTrue(reconcile(deck, named, target(2, "b", at = 0, seq = 3)).isEmpty())
        // A late report is no more of a request, however far from the deck.
        assertTrue(reconcile(deck, named, target(2, "b", at = 90_000, seq = 3)).isEmpty())
    }

    @Test
    fun theSameSongPlayedAgainStartsAgain() {
        val ops = reconcile(Decks(current = "a", positionMs = 60_000), target(1, "a", seq = 1), target(2, "a", at = 0, seq = 2))
        assertEquals(listOf(Op.Seek(0, SeekReason.Asked)), ops)
    }

    @Test
    fun aRoomsCorrectionIsFollowedOnlyWhenTheDeckHasReallyDrifted() {
        val previous = target(1, "a", at = 50_000, seq = 4)
        val close = reconcile(Decks(current = "a", positionMs = 50_800), previous, target(1, "a", at = 50_000, seq = 5))
        assertTrue(close.isEmpty())
        val adrift = reconcile(Decks(current = "a", positionMs = 40_000), previous, target(1, "a", at = 50_000, seq = 5))
        assertEquals(listOf(Op.Seek(50_000, SeekReason.Asked)), adrift)
    }

    @Test
    fun aTargetFromACoreWithoutTheCounterStillDecodes() {
        val old = CoreJson.decodeFromString(Target.serializer(), """{"Epoch":3,"VideoID":"a","StartAtMs":5,"Playing":true}""")
        assertEquals(0L, old.seekSeq)
        assertEquals("a", old.videoId)
        val new = CoreJson.decodeFromString(Target.serializer(), """{"VideoID":"a","SeekSeq":9}""")
        assertEquals(9L, new.seekSeq)
        // Without a named place a seek goes to the one position there is.
        assertEquals(null, old.seekToMs)
        assertEquals(5L, old.seekPositionMs)
        val named = CoreJson.decodeFromString(Target.serializer(), """{"StartAtMs":5,"SeekSeq":9,"SeekToMs":0}""")
        assertEquals(0L, named.seekPositionMs)
    }

    @Test
    fun theLevelSetsWhatTracksAreTurnedDownTo() {
        assertEquals(-14.0, lufsFor("normal"), 0.0)
        assertEquals(-19.0, lufsFor("quiet"), 0.0)
        assertEquals(-11.0, lufsFor("loud"), 0.0)
        assertEquals(-14.0, lufsFor("anything else"), 0.0)
        // 6 dB over the target is about half; quiet tracks stay as they are.
        assertEquals(0.5f, loudnessGain(-8.0, -14.0), 0.01f)
        assertEquals(1f, loudnessGain(-20.0, -14.0), 0f)
        assertEquals(1f, loudnessGain(null, -14.0), 0f)
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
    @Test
    fun aDeckOnAnAddressThatHasGoneLoadsAgain() {
        assertTrue(sourceMoved("127.0.0.1:4000", "127.0.0.1:5000"))
        assertFalse(sourceMoved("127.0.0.1:4000", "127.0.0.1:4000"))
        // A file on the phone has no address; a core still starting has none yet.
        assertFalse(sourceMoved(null, "127.0.0.1:5000"))
        assertFalse(sourceMoved("127.0.0.1:4000", null))
        // Counted as holding nothing, the deck is loaded from where the core says, not sought.
        val t = target(1, "a", at = 30_000)
        assertEquals(listOf(Op.Load(30_000)), reconcile(Decks(current = null, next = null), t, t))
    }

    private fun prepares(wasMeant: Boolean = true, failed: Boolean = false, idle: Boolean = true, holds: Boolean = true, meant: Boolean = true) =
        prepareAgain(idle, holds, meant, wasMeant, failed)

    @Test
    fun aDeckThatFailedIsPreparedWhenPlayIsPressed() {
        // A restored track that failed in the background, then play.
        assertTrue(prepares(wasMeant = false, failed = true))
    }

    @Test
    fun aDeckThatFailedIsPreparedUnderANewEpoch() {
        // Held offline, and started again by the core with the same id and position.
        assertTrue(prepares(wasMeant = true, failed = false))
    }

    @Test
    fun aFailureIsNotRetriedOnEachProjectionItBrings() {
        assertFalse(prepares(wasMeant = true, failed = true))
    }

    @Test
    fun onlyAnIdleDeckMeantToPlayItsTrackIsPrepared() {
        assertFalse(prepares(idle = false))
        assertFalse(prepares(holds = false))
        assertFalse(prepares(meant = false))
    }

    @Test
    fun aDeckPreparedAgainSeeksOnlyIfItHasLostItsPlace() {
        assertEquals(150_000L, placeAfterFailure(0, 150_000))
        assertNull(placeAfterFailure(150_100, 150_000))
    }

    @Test
    fun theWatchdogArmsOnAFreshLoadStartedWhileBuffering() {
        // No state change comes: it was buffering, held back, before play.
        assertEquals(5000L, waitingSince(true, 0, 5000))
        assertEquals(5000L, waitingSince(true, 5000, 6000))
    }

    @Test
    fun theWatchdogDisarmsWhenPausedOrPlaying() {
        assertEquals(0L, waitingSince(false, 5000, 6000))
    }

    @Test
    fun aSongStillBeingFoundIsNotAStall() {
        assertFalse(stalled(1000, 1000 + STALL_MS + 1, hadAudio = false))
        assertTrue(stalled(1000, 1000 + FIRST_AUDIO_STALL_MS + 1, hadAudio = false))
    }

    @Test
    fun aSongThatHadAudioStallsSoon() {
        assertFalse(stalled(1000, 1000 + STALL_MS, hadAudio = true))
        assertTrue(stalled(1000, 1000 + STALL_MS + 1, hadAudio = true))
        assertFalse(stalled(0, 1_000_000, hadAudio = true))
    }

    @Test
    fun theCoresOwnQueueHandedBackIsNotPlayedAgain() {
        val queue = listOf("a", "b", "c")
        assertEquals(Handed.SameQueue(null, null), handed(queue, 1, 90_000, queue, 1, 90_000))
        assertEquals(Handed.SameQueue(null, null), handed(queue, 1, 90_000, queue, 1, null))
    }

    @Test
    fun theCoresOwnQueueHandedBackMovesOnlyWhereAsked() {
        val queue = listOf("a", "b", "c")
        assertEquals(Handed.SameQueue(null, 30_000L), handed(queue, 1, 90_000, queue, 1, 30_000))
        assertEquals(Handed.SameQueue(2, null), handed(queue, 1, 90_000, queue, 2, null))
    }

    @Test
    fun anotherQueueHandedOverIsPlayed() {
        assertEquals(Handed.NewQueue, handed(listOf("a", "b"), 0, 0, listOf("b", "a"), 0, null))
        assertEquals(Handed.NewQueue, handed(emptyList(), 0, 0, listOf("a"), 0, null))
    }
}
