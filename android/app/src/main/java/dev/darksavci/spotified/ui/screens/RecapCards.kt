package dev.darksavci.spotified.ui.screens

import dev.darksavci.spotified.api.Stats
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.time.temporal.ChronoUnit

/** A span of listening as the recap asks for it: the exact seconds, and the days an older core goes by instead. */
internal data class RecapSpan(val year: Int, val from: Long, val to: Long, val days: Int, val running: Boolean)

/** [year] as a calendar year here, up to now if it is still running. */
internal fun recapSpan(year: Int, today: LocalDate, zone: ZoneId, now: Instant): RecapSpan {
    val start = LocalDate.of(year, 1, 1).atStartOfDay(zone).toInstant()
    val end = LocalDate.of(year + 1, 1, 1).atStartOfDay(zone).toInstant()
    val running = year == today.year
    return RecapSpan(
        year, start.epochSecond, (if (running) now else end).epochSecond,
        ChronoUnit.DAYS.between(LocalDate.of(year, 1, 1), today).toInt() + 1, running,
    )
}

/** The years to offer: this one and the one before. */
internal fun recapYears(today: LocalDate) = listOf(today.year, today.year - 1)

/** Which to open on: in January this year has hardly begun, so last year's. */
internal fun firstRecapYear(today: LocalDate) = if (today.monthValue == 1) today.year - 1 else today.year

internal fun hours(ms: Long): String {
    val minutes = ms / 60_000
    return when {
        minutes >= 60 -> "${minutes / 60} h ${minutes % 60} min"
        minutes == 0L && ms > 0 -> "under a minute"
        else -> "$minutes min"
    }
}

internal fun plays(n: Int) = if (n == 1) "1 play" else "$n plays"

/** One card of the story, as words and addresses, for the pager and for the picture alike. */
internal sealed interface RecapCard {
    data class Time(val value: String, val caption: String) : RecapCard
    data class Feature(val caption: String, val title: String, val subtitle: String, val art: String, val artistId: String = "", val round: Boolean = false) : RecapCard
    data class Top(val title: String, val rows: List<TopRow>) : RecapCard
}

internal data class TopRow(val title: String, val artist: String, val art: String)

/** The cards for a span's counts. None when nothing was played. */
internal fun recapCards(stats: Stats, span: RecapSpan): List<RecapCard> {
    if (stats.summary.plays == 0) return emptyList()
    val of = if (span.running) "so far this year" else "in ${span.year}"
    val suffix = if (span.running) "so far" else "of ${span.year}"
    return buildList {
        add(RecapCard.Time(hours(stats.summary.totalMs), "$of, across ${plays(stats.summary.plays)}."))
        stats.tracks.firstOrNull()?.let { add(RecapCard.Feature("Your top song $suffix", it.title, "${it.artist} • ${plays(it.plays)}", it.artwork)) }
        stats.artists.firstOrNull()?.let { add(RecapCard.Feature("Your top artist $suffix", it.artist, plays(it.plays), it.artwork, it.artistId, round = true)) }
        if (stats.tracks.isNotEmpty()) add(RecapCard.Top("Your top songs", stats.tracks.take(5).map { TopRow(it.title, it.artist, it.artwork) }))
    }
}
