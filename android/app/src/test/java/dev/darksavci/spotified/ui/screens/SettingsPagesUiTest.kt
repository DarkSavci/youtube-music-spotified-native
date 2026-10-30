package dev.darksavci.spotified.ui.screens

import dev.darksavci.spotified.api.Stats
import dev.darksavci.spotified.api.Summary
import dev.darksavci.spotified.api.TrackStat
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneOffset
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class SettingsSearchTest {
    @Test
    fun everyWordHasToAppearInAnyOfItsNames() {
        assertTrue(settingMatches("volume", "Volume boost", "Sound and quality", "louder"))
        assertTrue(settingMatches("  LOUD  sound ", "Volume boost", "Sound and quality", "louder"))
        assertFalse(settingMatches("volume cache", "Volume boost", "Sound and quality", "louder"))
        assertFalse(settingMatches("   ", "Volume boost"))
    }
}

class RecapTest {
    private val now = Instant.parse("2026-10-30T12:00:00Z")
    private val today = LocalDate.of(2026, 10, 30)

    @Test
    fun aPastYearIsTheWholeCalendarYear() {
        val span = recapSpan(2025, today, ZoneOffset.UTC, now)
        assertEquals(Instant.parse("2025-01-01T00:00:00Z").epochSecond, span.from)
        assertEquals(Instant.parse("2026-01-01T00:00:00Z").epochSecond, span.to)
        assertFalse(span.running)
    }

    @Test
    fun theYearRunningStopsAtNow() {
        val span = recapSpan(2026, today, ZoneOffset.UTC, now)
        assertEquals(Instant.parse("2026-01-01T00:00:00Z").epochSecond, span.from)
        assertEquals(now.epochSecond, span.to)
        assertTrue(span.running)
        assertEquals(303, span.days)
    }

    @Test
    fun itFollowsThePhonesOwnNewYear() {
        val span = recapSpan(2026, today, ZoneOffset.ofHours(3), now)
        assertEquals(Instant.parse("2025-12-31T21:00:00Z").epochSecond, span.from)
    }

    @Test
    fun januaryOpensOnTheYearJustEnded() {
        assertEquals(2025, firstRecapYear(LocalDate.of(2026, 1, 15)))
        assertEquals(2026, firstRecapYear(LocalDate.of(2026, 2, 1)))
        assertEquals(listOf(2026, 2025), recapYears(today))
    }

    @Test
    fun nothingPlayedMakesNoCards() {
        val stats = Stats(Summary(), emptyList(), emptyList(), emptyList(), emptyList())
        assertTrue(recapCards(stats, recapSpan(2026, today, ZoneOffset.UTC, now)).isEmpty())
    }

    @Test
    fun theCardsNameTheYearTheyAreOf() {
        val stats = Stats(Summary(plays = 2, totalMs = 3_600_000), listOf(TrackStat("id", "Song", "Artist", plays = 2)), emptyList(), emptyList(), emptyList())
        val past = recapCards(stats, recapSpan(2025, today, ZoneOffset.UTC, now))
        assertEquals(RecapCard.Time("1 h 0 min", "in 2025, across 2 plays."), past[0])
        assertEquals("Your top song of 2025", (past[1] as RecapCard.Feature).caption)
        val running = recapCards(stats, recapSpan(2026, today, ZoneOffset.UTC, now))
        assertEquals("so far this year, across 2 plays.", (running[0] as RecapCard.Time).caption)
    }
}

class PermissionTest {
    @Test
    fun onlyARefusalAndroidWillNotAskAboutAgainNeedsTheSettingsPage() {
        val slow = NO_QUESTION_MS * 10
        // The first refusal: Android starts being able to explain.
        assertFalse(permanentlyRefused(granted = false, couldExplainBefore = false, canExplainNow = true, answeredInMs = slow))
        // The second: it stops, and it could before.
        assertTrue(permanentlyRefused(granted = false, couldExplainBefore = true, canExplainNow = false, answeredInMs = slow))
        // A question dismissed without an answer: nothing was refused.
        assertFalse(permanentlyRefused(granted = false, couldExplainBefore = false, canExplainNow = false, answeredInMs = slow))
        // Refused long ago: the answer comes with no question shown.
        assertTrue(permanentlyRefused(granted = false, couldExplainBefore = false, canExplainNow = false, answeredInMs = 20))
        assertFalse(permanentlyRefused(granted = true, couldExplainBefore = true, canExplainNow = false, answeredInMs = slow))
    }
}
