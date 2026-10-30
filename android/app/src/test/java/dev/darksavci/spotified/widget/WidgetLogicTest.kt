package dev.darksavci.spotified.widget

import androidx.compose.ui.unit.DpSize
import androidx.compose.ui.unit.dp
import dev.darksavci.spotified.api.Track
import dev.darksavci.spotified.car.AutoSetup
import dev.darksavci.spotified.ui.PlaylistShortcuts
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class WidgetLogicTest {
    private fun track(id: String, playable: Boolean = true) = Track(id = id, title = id, playable = playable)

    @Test
    fun eachSizeTheWidgetDeclaresGetsItsOwnLayout() {
        PlayerLayout.entries.forEach { assertEquals(it, PlayerLayout.of(it.size)) }
    }

    @Test
    fun aSizeBetweenTheDeclaredOnesGetsTheLayoutItFits() {
        assertEquals(PlayerLayout.Compact, PlayerLayout.of(DpSize(150.dp, 300.dp)))
        assertEquals(PlayerLayout.Line, PlayerLayout.of(DpSize(250.dp, 80.dp)))
        assertEquals(PlayerLayout.WideLine, PlayerLayout.of(DpSize(400.dp, 80.dp)))
        assertEquals(PlayerLayout.Tall, PlayerLayout.of(DpSize(400.dp, 200.dp)))
    }

    @Test
    fun aRowPlaysTheSongItNamesEvenIfTheListHasMoved() {
        val drawn = listOf(track("a"), track("b"), track("c"))
        val since = listOf(track("new"), track("a"), track("b"), track("c"))
        assertEquals(1, startOf(drawn, "b"))
        assertEquals(2, startOf(since, "b"))
        assertNull(startOf(since, "gone"))
    }

    @Test
    fun aListShowsOnlyWhatPlaysAndEachSongOnce() {
        val rows = rowsOf(listOf(track("a"), track("b", playable = false), track("a"), track(""), track("c")))
        assertEquals(listOf("a", "c"), rows.map { it.id })
        assertEquals(WidgetLists.ROWS, rowsOf((1..40).map { track("t$it") }).size)
    }

    @Test
    fun onlyTheLastThreePlaylistsStay() {
        assertEquals(emptyList<String>(), PlaylistShortcuts.overflowing(listOf("a", "b"), "c"))
        assertEquals(listOf("c"), PlaylistShortcuts.overflowing(listOf("a", "b", "c"), "d"))
        // Opening one already there moves it up and takes nothing away.
        assertEquals(emptyList<String>(), PlaylistShortcuts.overflowing(listOf("a", "b", "c"), "c"))
    }

    @Test
    fun theCarStepsAreExplainedOnceAndNotToWhoeverIsAlreadyThere() {
        assertTrue(AutoSetup.shouldExplain(onCar = true, explained = false, reached = false))
        assertFalse(AutoSetup.shouldExplain(onCar = false, explained = false, reached = false))
        assertFalse(AutoSetup.shouldExplain(onCar = true, explained = true, reached = false))
        assertFalse(AutoSetup.shouldExplain(onCar = true, explained = false, reached = true))
    }
}
