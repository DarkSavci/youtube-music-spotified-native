package dev.darksavci.spotified.ui.player

import androidx.compose.ui.graphics.Color
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class PlayerLogicTest {
    @Test
    fun aQuickGestureIsMeasuredByWhereItWent() {
        // The tracker read nothing of a swipe of 300 pixels up in a tenth of a second.
        assertEquals(-3000f, fingerVelocity(0f, -300f, 100), 0.1f)
        // Nor can a misread of its direction close what is going up.
        assertTrue(fingerVelocity(5000f, -300f, 100) < 0f)
        // A finger held at a place and let go has stopped, whatever brought it there.
        assertEquals(0f, fingerVelocity(0f, -300f, 900), 0f)
        assertEquals(-800f, fingerVelocity(-800f, -300f, 900), 0f)
    }

    @Test
    fun keysHoldNoPlaceAndASongQueuedTwiceHasTwo() {
        assertEquals(listOf("a#0", "b#0", "a#1"), queueKeys(listOf("a", "b", "a")))
        // Moving a row leaves every other row its key.
        assertEquals(setOf("a#0", "b#0", "c#0"), queueKeys(listOf("c", "a", "b")).toSet())
    }

    @Test
    fun movedPutsTheElementAtItsNewPlace() {
        assertEquals(listOf("b", "c", "a"), listOf("a", "b", "c").moved(0, 2))
        assertEquals(listOf("c", "a", "b"), listOf("a", "b", "c").moved(2, 0))
        assertEquals(listOf("a", "b"), listOf("a", "b").moved(1, 5))
    }

    /** The same arithmetic as `move` in core/internal/session/core.go. */
    private fun coreMove(items: List<String>, from: Int, to: Int): List<String> {
        val rest = items.toMutableList()
        val item = rest.removeAt(from)
        rest.add(to, item)
        return rest
    }

    @Test
    fun aDropTellsTheCoreTheSongsFinalPlace() {
        val fresh = listOf("p", "now", "a", "b", "c", "d")
        for (ordered in listOf(
            listOf("p", "now", "b", "c", "a", "d"),
            listOf("p", "now", "b", "c", "d", "a"),
            listOf("p", "now", "b", "a", "c", "d"),
        )) {
            val to = dropPlace(fresh, ordered, "a", playing = 1)!!
            assertEquals(ordered, coreMove(fresh, 2, to))
        }
        assertEquals(listOf("p", "now", "d", "a", "b", "c"), coreMove(fresh, 5, dropPlace(fresh, listOf("p", "now", "d", "a", "b", "c"), "d", 1)!!))
    }

    @Test
    fun aSongCannotBeDroppedAboveTheOnePlaying() {
        val fresh = listOf("p", "now", "a", "b")
        // Left at the very top, it still lands just below the song playing.
        assertEquals(2, dropPlace(fresh, listOf("b", "p", "now", "a"), "b", playing = 1))
        // Played rows and the playing row are not moved at all.
        assertNull(dropPlace(fresh, listOf("now", "p", "a", "b"), "p", playing = 1))
        assertNull(dropPlace(fresh, fresh, "now", playing = 1))
    }

    @Test
    fun aDropThatChangesNothingSendsNothing() {
        val fresh = listOf("now", "a", "b")
        assertNull(dropPlace(fresh, fresh, "a", playing = 0))
        assertNull(dropPlace(fresh, listOf("now", "a"), "gone", playing = 0))
    }

    @Test
    fun aDropStillWorksWhenTheQueueMovedOnDuringTheDrag() {
        // The song played on and a new one arrived while the finger was down.
        val fresh = listOf("now", "next", "a", "b", "c", "new")
        val ordered = listOf("now", "a", "b", "c", "next")
        val to = dropPlace(fresh, ordered, "next", playing = 0)!!
        assertEquals(listOf("now", "a", "b", "c", "next", "new"), coreMove(fresh, 1, to))
    }

    @Test
    fun edgeScrollOnlyInTheZonesAndFasterFurtherIn() {
        val zone = 100f
        assertEquals(0f, edgeScroll(200f, 300f, 0f, 1000f, zone, 20f), 0.001f)
        assertEquals(-10f, edgeScroll(50f, 150f, 0f, 1000f, zone, 20f), 0.001f)
        assertEquals(-20f, edgeScroll(-40f, 60f, 0f, 1000f, zone, 20f), 0.001f)
        assertEquals(10f, edgeScroll(850f, 950f, 0f, 1000f, zone, 20f), 0.001f)
        assertEquals(20f, edgeScroll(1000f, 1100f, 0f, 1000f, zone, 20f), 0.001f)
    }

    @Test
    fun aFlickDecidesHowASheetSettlesAndOtherwiseTheNearest() {
        assertFalse(settlesOpen(0.9f, 2000f, 500f))
        assertTrue(settlesOpen(0.1f, -2000f, 500f))
        // Coming up from closed, a fifth of the way carries on.
        assertTrue(settlesOpen(0.3f, 0f, 500f, closeBelow = SHEET_TRAVEL))
        assertFalse(settlesOpen(0.1f, 0f, 500f, closeBelow = SHEET_TRAVEL))
        assertTrue(settlesOpen(0.9f, 100f, 500f))
        // A slow pull past a fifth of the way carries on.
        assertFalse(settlesOpen(0.7f, 100f, 500f))
    }

    @Test
    fun whiteReadsOnAnyCoversColour() {
        for (cover in listOf(Color(0xFF9E9E9E), Color(0xFFF2D21B), Color.White, Color(0xFFE8E8F0), Color(0xFF39FF14))) {
            assertTrue("$cover", contrastWithWhite(readableBehindWhite(cover)) >= 4.5)
            assertTrue("$cover", contrastWithWhite(readableBehindWhite(cover, 7.0)) >= 7.0)
        }
    }

    @Test
    fun aDarkEnoughColourIsLeftAloneAndAHueIsKept() {
        val dark = Color(0xFF7A1520)
        assertEquals(dark, readableBehindWhite(dark))
        val shaded = readableBehindWhite(Color(0xFFF2D21B))
        assertTrue(shaded.red > shaded.green && shaded.green > shaded.blue)
    }

    @Test
    fun whiteOnBlackIsTheMost() {
        assertEquals(21.0, contrastWithWhite(Color.Black), 0.01)
        assertEquals(1.0, contrastWithWhite(Color.White), 0.01)
    }

    @Test
    fun opennessRunsFromClosedToOpen() {
        assertEquals(1f, opennessAt(0f, 1000f), 0.001f)
        assertEquals(0f, opennessAt(1000f, 1000f), 0.001f)
        assertEquals(0.25f, opennessAt(750f, 1000f), 0.001f)
        assertEquals(0f, opennessAt(Float.NaN, 1000f), 0.001f)
    }

    @Test
    fun theSeekBarHoldsUntilTheEngineArrives() {
        val hold = SeekHold(near = 750, patience = 4000)
        assertEquals(10_000, hold.shown(10_000, now = 0))
        hold.sent(60_000, now = 0)
        // The old position is still being reported: it does not snap back.
        assertEquals(60_000, hold.shown(10_250, now = 250))
        // The engine arrives, and the bar follows it again.
        assertEquals(60_100, hold.shown(60_100, now = 500))
        assertEquals(61_000, hold.shown(61_000, now = 1000))
    }

    @Test
    fun theSeekBarGivesUpOnASeekThatNeverArrives() {
        val hold = SeekHold(near = 750, patience = 4000)
        hold.sent(60_000, now = 0)
        assertEquals(60_000, hold.shown(10_000, now = 3000))
        assertEquals(11_000, hold.shown(11_000, now = 5000))
    }

    @Test
    fun aCountdownRoundsUpSoItNeverSaysNoTimeLeftEarly() {
        assertEquals("0:01", countdown(1))
        assertEquals("12:05", countdown(725_000))
        assertEquals("1:00:00", countdown(3_600_000))
    }
}
