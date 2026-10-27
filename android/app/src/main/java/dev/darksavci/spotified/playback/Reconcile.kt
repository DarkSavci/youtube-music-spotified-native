package dev.darksavci.spotified.playback

import dev.darksavci.spotified.api.Target
import kotlin.math.abs

/*
 * What the decks must do to reach the core's target. A port of reconcile in
 * crates/audio/src/engine.rs, kept pure so it is tested without a player.
 */

/** How far a deck may be from the target's position before it seeks: less is drift between two clocks. */
const val SEEK_TOLERANCE_MS = 1500L

/** The decks as [reconcile] needs to see them. */
data class Decks(
    /** The track on the playing deck. */
    val current: String? = null,
    val positionMs: Long = 0,
    /** The playing deck has run to its end. */
    val ended: Boolean = false,
    /** The track on the preloaded deck. */
    val next: String? = null,
)

sealed interface Op {
    /** Nothing is to play: drop both decks. */
    object Stop : Op
    /** Load the target's track on the playing deck from this position. */
    data class Load(val startMs: Long) : Op
    /** The preloaded deck holds the target's track: make it the playing one. */
    object Promote : Op
    data class Seek(val positionMs: Long) : Op
    /** Load the target's next track on the preload deck. */
    object Preload : Op
    object DropPreload : Op
}

/** What must change for the decks to match [target]; [previous] is the target last acted on. */
fun reconcile(decks: Decks, previous: Target, target: Target): List<Op> {
    if (target.videoId.isEmpty()) return listOf(Op.Stop)
    val ops = mutableListOf<Op>()
    var next = decks.next
    if (decks.current != target.videoId) {
        if (next == target.videoId) {
            ops += Op.Promote
            next = null
            if (target.startAtMs > SEEK_TOLERANCE_MS) ops += Op.Seek(target.startAtMs)
        } else {
            ops += Op.Load(target.startAtMs)
        }
    } else if (previous.videoId == target.videoId) {
        // The same track under a new epoch, after it ended, is the core
        // starting it again: repeat one.
        val restarted = decks.ended && target.epoch != previous.epoch
        val moved = abs(decks.positionMs - target.startAtMs) > SEEK_TOLERANCE_MS
        if (restarted || moved) ops += Op.Seek(target.startAtMs)
    }
    // Otherwise the deck already plays a track the core has only now named:
    // the engine ran into it gaplessly. However far in it is by the time the
    // core catches up, it is left playing.
    val wanted = target.preloadVideoId.ifEmpty { null }
    when {
        next != null && next == wanted -> Unit
        wanted != null -> ops += Op.Preload
        next != null -> ops += Op.DropPreload
    }
    return ops
}

/**
 * Whether it is time to start fading into the next track: the fade's length
 * before the end. A track too short to hold a fade at each end ends plainly.
 */
fun crossfadeDue(positionMs: Long, durationMs: Long, crossfadeMs: Long, speed: Float): Boolean {
    val fadeMs = (crossfadeMs * speed).toLong()
    return crossfadeMs > 0 && durationMs > fadeMs * 2 && positionMs + fadeMs >= durationMs
}

/**
 * The volume slider's position as a gain: loudness is heard on a curve, so
 * the lower half of the slider gets more of the range. Past 1 is boost,
 * which a phone's player cannot give; it is held at 1.
 */
fun gainFor(volume: Float): Float =
    if (volume <= 1f) Math.pow(volume.coerceAtLeast(0f).toDouble(), 1 / 0.6).toFloat() else 1f
