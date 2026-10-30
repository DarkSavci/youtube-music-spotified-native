package dev.darksavci.spotified.ui.player

import androidx.compose.ui.graphics.Color
import java.util.Locale
import kotlin.math.abs
import kotlin.math.pow

/*
 * The arithmetic of the player's gestures, apart from the screens so that it can
 * be tested: which way a let-go sheet goes, where a dragged song lands, and what
 * the seek bar shows while the engine catches up.
 */

/**
 * A stable key for each song in a queue: its id, and which time this is that it
 * appears, since a song can be queued twice. The place in the queue is not in the
 * key, so that a moved row keeps its key and only that row moves.
 */
internal fun queueKeys(ids: List<String>): List<String> {
    val seen = HashMap<String, Int>()
    return ids.map { id ->
        val time = seen[id] ?: 0
        seen[id] = time + 1
        "$id#$time"
    }
}

/** This list with the element at [from] taken out and put back so that it ends at [to]. */
internal fun <T> List<T>.moved(from: Int, to: Int): List<T> {
    if (from == to || from !in indices || to !in indices) return this
    val out = toMutableList()
    out.add(to, out.removeAt(from))
    return out
}

/**
 * Where to tell the core a dragged song goes: the `To` of its move, which is the
 * song's place once moved. [ordered] is the list as the listener left it and
 * [fresh] the queue as it is now, which may have changed during the drag; the song
 * goes after whichever song it was left behind. It never lands on or above the
 * song at [playing]. Null when it is not there, is not below the song playing, or
 * has not moved.
 */
internal fun dropPlace(fresh: List<String>, ordered: List<String>, key: String, playing: Int): Int? {
    val from = fresh.indexOf(key)
    val left = ordered.indexOf(key)
    if (from < 0 || left < 0 || from <= playing) return null
    val rest = fresh.filter { it != key }
    var after = -1
    for (i in left - 1 downTo 0) {
        after = rest.indexOf(ordered[i])
        if (after >= 0) break
    }
    // Nothing above the song playing changes, so its place in the rest is the same.
    val to = maxOf(after + 1, playing + 1).coerceAtMost(rest.size)
    return to.takeIf { it != from }
}

/**
 * How far to scroll a list this frame for a dragged row whose top is at [top] and
 * bottom at [bottom], in a list showing [start] to [end]: nothing outside the
 * [zone] at each edge, then faster the further in, up to [fastest].
 */
internal fun edgeScroll(top: Float, bottom: Float, start: Float, end: Float, zone: Float, fastest: Float): Float {
    if (zone <= 0f) return 0f
    val intoTop = (start + zone - top) / zone
    val intoBottom = (bottom - (end - zone)) / zone
    return when {
        intoTop > 0f -> -fastest * intoTop.coerceAtMost(1f)
        intoBottom > 0f -> fastest * intoBottom.coerceAtMost(1f)
        else -> 0f
    }
}

/**
 * Whether an open sheet let go should stay open: a fast enough flick decides, either
 * way, and otherwise whether it has been pulled past [closeBelow]. [open] runs from 0
 * closed to 1 open, and a positive [velocity] is downwards, towards closed.
 */
internal fun settlesOpen(open: Float, velocity: Float, fast: Float, closeBelow: Float = 1f - SHEET_TRAVEL): Boolean = when {
    velocity > fast -> false
    velocity < -fast -> true
    else -> open >= closeBelow
}

/**
 * How fast a finger was going when let go. A gesture over in a moment leaves the
 * tracker too few points to read, and it often answers nothing at all, however fast;
 * so a quick gesture is measured by where it went, and one that took longer by the tracker.
 */
internal fun fingerVelocity(tracked: Float, travelled: Float, tookMs: Long): Float =
    if (tookMs in 1..QUICK_GESTURE_MS && travelled != 0f) travelled / tookMs * 1000f else tracked

private const val QUICK_GESTURE_MS = 300L

/** How much of the way a slow drag must take a sheet, in either direction, for it to carry on. */
internal const val SHEET_TRAVEL = 0.2f

/** How open a sheet is at [offset], where 0 is open and [closedAt] is closed. */
internal fun opennessAt(offset: Float, closedAt: Float): Float =
    if (offset.isNaN() || closedAt <= 0f) 0f else (1f - offset / closedAt).coerceIn(0f, 1f)

/**
 * What the seek bar shows after a seek: where it was let go, until the engine has
 * reached it or plainly is not going to, rather than going back to the old position
 * in between.
 */
internal class SeekHold(private val near: Long = 750, private val patience: Long = 4000) {
    private var target: Long? = null
    private var since = 0L

    fun sent(positionMs: Long, now: Long) {
        target = positionMs
        since = now
    }

    fun shown(positionMs: Long, now: Long): Long {
        val held = target ?: return positionMs
        if (abs(positionMs - held) <= near || now - since > patience) {
            target = null
            return positionMs
        }
        return held
    }
}

/** How long until something happens, as a clock: "12:05". */
internal fun countdown(ms: Long): String {
    val s = (ms.coerceAtLeast(0) + 999) / 1000
    return if (s >= 3600) "%d:%02d:%02d".format(Locale.US, s / 3600, s / 60 % 60, s % 60) else "%d:%02d".format(Locale.US, s / 60, s % 60)
}

private fun linear(channel: Float): Double =
    if (channel <= 0.03928f) channel / 12.92 else ((channel + 0.055) / 1.055).pow(2.4)

/** The contrast of white on [color], as WCAG counts it: from 1, none, to 21, on black. */
internal fun contrastWithWhite(color: Color): Double {
    val light = 0.2126 * linear(color.red) + 0.7152 * linear(color.green) + 0.0722 * linear(color.blue)
    return 1.05 / (light + 0.05)
}

/**
 * [color] darkened, keeping its hue, until white reads on it at [contrast] or better.
 * A cover's colour can be anything, and pale grey or yellow would lose the words.
 * A colour that is dark enough already comes back as it is.
 */
internal fun readableBehindWhite(color: Color, contrast: Double = 4.5): Color {
    var kept = 1f
    var shaded = color
    while (contrastWithWhite(shaded) < contrast && kept > 0f) {
        kept = (kept - 0.02f).coerceAtLeast(0f)
        shaded = Color(color.red * kept, color.green * kept, color.blue * kept, color.alpha)
    }
    return shaded
}
