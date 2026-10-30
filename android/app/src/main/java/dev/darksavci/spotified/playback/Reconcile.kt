package dev.darksavci.spotified.playback

import dev.darksavci.spotified.api.Target
import kotlin.math.abs
import kotlin.math.min
import kotlin.math.pow

/*
 * What the decks must do to reach the core's target. A port of reconcile in
 * crates/audio/src/engine.rs, kept pure so it is tested without a player.
 */

/**
 * How far a deck may be from where a seek names before it seeks. The core's seek
 * counter also moves with every follow of a room, for a song added or a pause as
 * much as for a correction, and seeking for those would be heard as a hiccup each
 * time. It has to stay under the room's drift (DRIFT_MS in together/Sync.kt), by
 * enough for the command's way there, or a player brought back would not move.
 */
const val SEEK_TOLERANCE_MS = 900L

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

/** Why a seek is being made, for the log. */
enum class SeekReason(val label: String) {
    /** The core moved the position: its seek counter changed. */
    Asked("the core asked"),
    /** The track ended and the core started it again: repeat one. */
    Restart("started again after its end"),
    /** A preloaded track took over, but is to start further in. */
    Promoted("promoted to a later start"),
}

sealed interface Op {
    /** Nothing is to play: drop both decks. */
    object Stop : Op
    /** Load the target's track on the playing deck from this position. */
    data class Load(val startMs: Long) : Op
    /** The preloaded deck holds the target's track: make it the playing one. */
    object Promote : Op
    data class Seek(val positionMs: Long, val reason: SeekReason) : Op
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
            if (target.startAtMs > SEEK_TOLERANCE_MS) ops += Op.Seek(target.startAtMs, SeekReason.Promoted)
        } else {
            ops += Op.Load(target.startAtMs)
        }
    } else if (previous.videoId == target.videoId) {
        // The same track under a new epoch, after it ended, is the core
        // starting it again: repeat one.
        val restarted = decks.ended && target.epoch != previous.epoch
        // Only the core's counter asks for a seek, and it goes where the seek
        // named. The position alone also echoes the deck's own reports, so it
        // can have moved on by the time the seek is seen, and following an echo
        // pulled songs back.
        val asked = target.seekSeq != previous.seekSeq &&
            abs(decks.positionMs - target.seekPositionMs) > SEEK_TOLERANCE_MS
        when {
            restarted -> ops += Op.Seek(target.seekPositionMs, SeekReason.Restart)
            asked -> ops += Op.Seek(target.seekPositionMs, SeekReason.Asked)
        }
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
 * Whether a deck reading from the core must load again: the core restarted on
 * another port, so the address it holds answers no more. A file on the phone has
 * no address and is not moved; [now] is null while the core is starting.
 */
fun sourceMoved(loadedFrom: String?, now: String?): Boolean =
    loadedFrom != null && now != null && loadedFrom != now

/**
 * Whether the playing deck, which holds the target's track but stands idle after
 * an error, is prepared again. A deck that has already failed under this epoch is
 * not tried again until the music is started anew: the failure was reported, the
 * report brings another projection, and retrying on each would never end.
 */
fun prepareAgain(idle: Boolean, holdsTarget: Boolean, meantToPlay: Boolean, wasMeantToPlay: Boolean, failedThisEpoch: Boolean): Boolean =
    idle && holdsTarget && meantToPlay && (!wasMeantToPlay || !failedThisEpoch)

/** Where a deck prepared again must seek to first, if it has lost its place. */
fun placeAfterFailure(deckMs: Long, targetMs: Long): Long? =
    targetMs.takeIf { abs(deckMs - targetMs) > SEEK_TOLERANCE_MS }

/** How long a deck may wait for audio before it counts as stalled, in the middle of a song. */
const val STALL_MS = 8000L

/**
 * The same for a song that has given no audio yet. Its address may still be being
 * worked out, which can take yt-dlp longer than a stall, and reading again would
 * drop that request and start it over.
 */
const val FIRST_AUDIO_STALL_MS = 30_000L

/** When a deck began waiting for audio it means to play, carried from [since]; 0 when it is not waiting. */
fun waitingSince(meantToPlayAndBuffering: Boolean, since: Long, now: Long): Long = when {
    !meantToPlayAndBuffering -> 0
    since == 0L -> now
    else -> since
}

/** Whether a deck waiting since [since] has waited too long. */
fun stalled(since: Long, now: Long, hadAudio: Boolean): Boolean =
    since != 0L && now - since > (if (hadAudio) STALL_MS else FIRST_AUDIO_STALL_MS)

/** What to do when a system surface hands over a queue to play. */
sealed interface Handed {
    /** It is not the core's queue: play it. */
    object NewQueue : Handed

    /** It is the core's queue: leave it as it is, moved only where asked, and not started again. */
    data class SameQueue(val jumpTo: Int?, val seekTo: Long?) : Handed
}

/**
 * Reads what the media session was handed. Resuming from the system's controls
 * hands back the core's own queue; playing that again would restart the song and
 * shuffle the queue anew. [startMs] is null when no place was named.
 */
fun handed(queue: List<String>, index: Int, positionMs: Long, items: List<String>, at: Int, startMs: Long?): Handed {
    if (items != queue) return Handed.NewQueue
    val moved = at != index
    return Handed.SameQueue(
        jumpTo = at.takeIf { moved },
        seekTo = startMs?.takeIf { moved || abs(it - positionMs) > SEEK_TOLERANCE_MS },
    )
}

/**
 * Whether it is time to start fading into the next track: the fade's length
 * before the end. A track too short to hold a fade at each end ends plainly.
 */
fun crossfadeDue(positionMs: Long, durationMs: Long, crossfadeMs: Long, speed: Float): Boolean {
    val fadeMs = (crossfadeMs * speed).toLong()
    return crossfadeMs > 0 && durationMs > fadeMs * 2 && positionMs + fadeMs >= durationMs
}

/** The loudness tracks are turned down to for each Volume level, in LUFS, as on the desktop. */
fun lufsFor(level: String): Double = when (level) {
    "quiet" -> -19.0
    "loud" -> -11.0
    else -> -14.0
}

/** Loud tracks are turned down to the target; quiet ones are never turned up. */
fun loudnessGain(lkfs: Double?, targetLufs: Double): Float {
    if (lkfs == null) return 1f
    val db = min(0.0, targetLufs - lkfs)
    return 10.0.pow(db / 20).toFloat()
}
