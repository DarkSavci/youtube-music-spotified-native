package dev.darksavci.spotified.together

import dev.darksavci.spotified.api.Command
import dev.darksavci.spotified.api.Track
import kotlin.math.abs
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put

/*
 * Keeping this player with the room, and the room's controls with this
 * player's buttons. Both are plain functions of what the room says and what
 * the player is doing. Mirrors crates/app/src/together/sync.rs.
 */

/** How far this player may be from the room before it is brought back. */
private const val DRIFT_MS = 1000L

/** How long after one correction the next may come, so a slow player is not sought over and over. */
private const val BETWEEN_CORRECTIONS_MS = 4000L

/** Past this far into a song, "previous" starts it again instead. */
const val RESTART_AFTER_MS = 3000L

/** What this player is doing. */
data class Local(
    /** Whether the core is following a room at all yet. */
    val following: Boolean,
    val queue: List<String>,
    val current: String?,
    val positionMs: Long,
    /** Playing, or meaning to. */
    val playing: Boolean,
    /** Neither loading nor stalled: its position can be trusted. */
    val settled: Boolean,
)

/**
 * The command that brings the player to the room, if it is not there:
 * another song, another queue, the other of playing and paused, or too far
 * from where the room is in the song. [sinceCorrectionMs] is how long ago
 * the player was last brought back, if it has been.
 */
fun follow(room: Room, serverNow: Double, local: Local, appliedEntry: String?, sinceCorrectionMs: Long?): Command.FollowRoom? {
    val current = room.currentEntry()
        // An empty room still takes the player: its own queue is put by.
        ?: return if (!local.following) Command.FollowRoom(emptyList(), 0, "", 0, false) else null
    val (index, entry) = current
    val positionMs = room.positionAt(serverNow)
    val anotherSong = appliedEntry != entry.id || local.current != entry.track.id
    val anotherQueue = local.queue != room.queue.map { it.track.id }
    val adrift = local.settled &&
        abs(local.positionMs - positionMs) > DRIFT_MS &&
        (sinceCorrectionMs == null || sinceCorrectionMs >= BETWEEN_CORRECTIONS_MS)
    val wanted = !local.following || anotherSong || anotherQueue || local.playing != room.playing || adrift
    return if (wanted) Command.FollowRoom(room.tracks(), index, entry.id, positionMs, room.playing) else null
}

/** How this player stands with the room: what the page says of it, and what the room's other listeners are told. */
enum class Standing(val label: String) {
    Joining("Ready to join"),

    /** This player reached the song's end before the room did. */
    WaitingForNext("Waiting for the next song"),
    Buffering("Buffering"),
    CatchingUp("Catching up"),
    InSync("In sync"),
    Paused("Paused together"),

    /** The room's song cannot be played here: YouTube refuses this device for now, or will not give this song at all. */
    Unavailable("Track unavailable");

    /** What the relay calls it. */
    fun wire(roomPlaying: Boolean): String = when (this) {
        Buffering -> "buffering"
        CatchingUp -> "catching up"
        // The relay moves the room on when everyone says so of the same song.
        Unavailable -> "unavailable"
        else -> if (roomPlaying) "listening" else "paused"
    }

    companion object {
        fun of(room: Room, serverNow: Double, local: Local, endedHere: Boolean): Standing {
            val adrift = abs(local.positionMs - room.positionAt(serverNow)) > DRIFT_MS
            return when {
                endedHere && room.playing -> WaitingForNext
                !local.settled -> Buffering
                adrift && room.playing -> CatchingUp
                room.playing -> InSync
                else -> Paused
            }
        }
    }
}

/**
 * Whether the room should be told this player's measure of a song the room
 * has down as [listedMs] long: when it has no length for it, or one that is
 * out by more than a second. A measure wildly unlike the room's is another
 * recording, and says nothing of the room's.
 */
fun lengthWorthTelling(listedMs: Long, measuredMs: Long): Boolean =
    measuredMs > 0 && (listedMs == 0L || abs(measuredMs - listedMs) in 1001L..15_000L)

/** Where one of the player's own commands goes while it is in a room. */
sealed interface Routed {
    /** To the room, as this command of its own. */
    data class ToRoom(val kind: String, val fields: JsonObject) : Routed

    /** To the core, as outside a room. */
    object Core : Routed

    /** Nowhere, with a sentence saying why. */
    data class Refused(val why: String) : Routed
}

const val LEADER_PLAYS = "The leader controls playback in this room."
const val LISTEN_ONLY = "This room is listen only."
const val LEADER_SETS = "Only the leader can change room settings."
const val NO_SHUFFLE = "Choose First in, first out or Take turns in room settings."
const val NOT_AHEAD = "Only playback controllers may insert ahead of others."
const val OWN_ONLY = "You may only edit your upcoming contributions."
const val LEADER_ORDERS = "The leader controls queue order."

/** Said when a command held more songs than the room takes at once. */
const val FIRST_HUNDRED = "Using the first 100 songs. Add more in batches from your library."

/** Whether [command] holds more songs than go to the room in one go. */
fun trimmed(command: Command): Boolean = when (command) {
    is Command.Enqueue -> command.tracks.size > MOST_TRACKS
    is Command.Play -> (command.tracks.size - command.startIndex).coerceAtLeast(0) > MOST_TRACKS
    else -> false
}

/**
 * The room's command that adds [tracks] for [me]: into the queue, before
 * the entry [before] when that is given, or as requests where the leader
 * approves what guests add. Which it became in the end is the relay's to
 * say: the leader may change the rules while this is on its way.
 */
fun adding(room: Room, me: String, tracks: List<Track>, before: String?): Routed {
    if (room.requesting(me)) {
        // A request has no place yet; the leader chooses where it goes.
        return Routed.ToRoom("request", buildJsonObject { put("tracks", tracksJson(tracks)) })
    }
    if (!room.mayAdd(me)) return Routed.Refused(LISTEN_ONLY)
    if (before != null && !room.mayControl(me)) return Routed.Refused(NOT_AHEAD)
    return Routed.ToRoom("enqueue", buildJsonObject {
        put("tracks", tracksJson(tracks))
        put("before", before?.let(::JsonPrimitive) ?: JsonNull)
    })
}

private fun fields(vararg pairs: Pair<String, Any?>): JsonObject = buildJsonObject {
    for ((name, value) in pairs) {
        when (value) {
            null -> put(name, JsonNull)
            is String -> put(name, value)
            is Boolean -> put(name, value)
            is Number -> put(name, value)
            else -> error("not a JSON value: $value")
        }
    }
}

/** Turns a press of the player's controls into the room's command for it. The room then tells everyone, this player among them. */
fun route(command: Command, room: Room, me: String, positionMs: Long): Routed {
    val entryAt = { index: Int -> room.queue.getOrNull(index)?.id }
    val steers = room.mayControl(me)
    val guestAdds = !steers && room.mode == Mode.Contributions
    return when {
        // What the room itself asked of the player, and what is the
        // player's alone, go straight through.
        command is Command.FollowRoom || command is Command.LeaveRoom || command is Command.SetVolume -> Routed.Core
        command is Command.SetShuffle -> Routed.Refused(NO_SHUFFLE)
        command is Command.SetRepeat && !room.leads(me) -> Routed.Refused(LEADER_SETS)
        command is Command.SetRepeat -> Routed.ToRoom("settings", fields("repeat" to room.repeat.next().wire))
        // "Next" with nothing after the current song is an ordinary add.
        command is Command.Enqueue -> {
            val before = command.at?.let(entryAt)?.takeIf { steers || room.nextEntry() != null }
            adding(room, me, command.tracks, before)
        }
        // A guest who may only add songs asks for the one they picked,
        // rather than being told they cannot start it.
        command is Command.Play && guestAdds -> when (val picked = command.tracks.getOrNull(command.startIndex)) {
            null -> Routed.Refused(LEADER_PLAYS)
            else -> adding(room, me, listOf(picked), null)
        }
        command is Command.Remove && !steers -> {
            val own = room.queue.getOrNull(command.index)?.let { room.mayRemove(me, it) } ?: false
            if (own) Routed.ToRoom("remove", fields("entry" to entryAt(command.index)))
            else Routed.Refused(if (guestAdds) OWN_ONLY else LISTEN_ONLY)
        }
        command is Command.Move && !steers -> Routed.Refused(LEADER_ORDERS)
        !steers -> Routed.Refused(LEADER_PLAYS)
        command is Command.Toggle -> Routed.ToRoom(if (room.playing) "pause" else "play", fields())
        command is Command.Next -> Routed.ToRoom("next", fields())
        command is Command.Previous -> Routed.ToRoom("previous", fields("restart" to (positionMs > RESTART_AFTER_MS)))
        command is Command.Seek -> Routed.ToRoom("seek", fields("positionMs" to command.positionMs))
        command is Command.Jump -> Routed.ToRoom("jump", fields("entry" to entryAt(command.index)))
        command is Command.Remove -> Routed.ToRoom("remove", fields("entry" to entryAt(command.index)))
        command is Command.Move -> {
            // The room names a place by the entry that will follow it.
            val before = room.queue.filterIndexed { index, _ -> index != command.from }.getOrNull(command.to)?.id
            Routed.ToRoom("move", fields("entry" to entryAt(command.from), "before" to before))
        }
        command is Command.Play -> {
            val from = command.startIndex.coerceIn(0, command.tracks.size)
            val end = (from + MOST_TRACKS).coerceAtMost(command.tracks.size)
            Routed.ToRoom("replace", buildJsonObject { put("tracks", tracksJson(command.tracks.subList(from, end))) })
        }
        else -> Routed.Refused(LEADER_PLAYS)
    }
}
