package dev.darksavci.spotified.together

import dev.darksavci.spotified.api.Track
import kotlin.math.abs

/*
 * The room's rules as this app knows them: who may do what, and what to
 * say when the room changes. The relay decides; these mirror it, so that a
 * listener gets the answer here rather than a refused round trip for every
 * press. Mirrors crates/app/src/together/rules.rs.
 */

/** The most songs a room's queue holds. */
private const val QUEUE_MOST = 500

/** The most radio that waits in a queue at once. */
private const val RADIO_MOST = 50

/** Fewer songs than this after the current one, and the leader adds radio. */
const val RADIO_LOW = 5

/** How many songs a room is remembered to have had. */
private const val HEARD_MOST = 1000

fun Room.leads(member: String): Boolean = owner == member

/** Whether [member] answers song requests: the leader, or a DJ. */
fun Room.answers(member: String): Boolean = leads(member) || this.member(member)?.role == "dj"

/**
 * Whether what [member] adds waits for approval instead of queueing: a
 * guest in a room that takes requests, unless it accepts them all.
 */
fun Room.requesting(member: String): Boolean = mode == Mode.Contributions && !autoAccept && !mayControl(member)

/** Whether [member] may add songs at all: in a listen-only room only those who steer do. */
fun Room.mayAdd(member: String): Boolean = mayControl(member) || mode == Mode.Contributions

/**
 * Whether [member] may take [entry] out of the queue: those who steer any
 * but the current one, a guest only what they added themselves.
 */
fun Room.mayRemove(member: String, entry: Entry): Boolean =
    current != entry.id && (mayControl(member) || (mode == Mode.Contributions && entry.addedBy.id == member))

/** Whether [member] may take back the last edit: whoever made it, or the leader, for as long as the relay would allow it. */
fun Room.mayUndo(member: String, serverNow: Double): Boolean {
    val undo = undo ?: return false
    return undo.revision == revision && (undo.by == member || leads(member)) && serverNow < undo.expires
}

/** The queue from the current song on, with each entry's place in it. */
fun Room.upcoming(): List<IndexedValue<Entry>> {
    val from = currentEntry()?.index ?: 0
    return queue.withIndex().drop(from)
}

/** The requests [member] sees: all of them for those who answer, a guest only their own. */
fun Room.requestsFor(member: String): List<SongRequest> {
    val all = answers(member)
    return requests.filter { all || it.by.id == member }
}

/** How many of those who are connected have answered the ready check, and how many are connected. */
fun Room.ready(): Pair<Int, Int> {
    val connected = members.filter { it.connected }
    return connected.count { it.ready } to connected.size
}

/** The entries after the current one. */
private fun Room.afterCurrent(): List<Entry> {
    val from = currentEntry()?.let { it.index + 1 } ?: 0
    return queue.subList(from.coerceAtMost(queue.size), queue.size)
}

/**
 * How many radio songs the room will still take. Radio is nobody's pick,
 * so it is held to the room's size and to how much of it may wait at once,
 * not to anyone's limit.
 */
fun Room.radioAllowance(): Int {
    val space = (QUEUE_MOST - queue.size).coerceAtLeast(0).coerceAtMost(100)
    val waiting = afterCurrent().count { it.radio }
    return space.coerceAtMost((RADIO_MOST - waiting).coerceAtLeast(0))
}

/** How many songs wait after the current one. */
fun Room.left(): Int = afterCurrent().size

/** The entry after the current one, which "play next" goes before. */
fun Room.nextEntry(): Entry? = afterCurrent().firstOrNull()

/**
 * Notes the songs the room has had, played or jumped past, most recent
 * last. The relay refuses radio that repeats them; leaving them out here as
 * well keeps a radio batch from shrinking to nothing once it has.
 */
fun rememberHeard(heard: MutableList<String>, room: Room) {
    val played = room.currentEntry()?.let { it.index + 1 } ?: 0
    val recent = (room.history.size - 50).coerceAtLeast(0)
    val had = room.history.drop(recent) + room.queue.take(played.coerceAtMost(room.queue.size))
    for (entry in had) {
        heard.remove(entry.track.id)
        heard.add(entry.track.id)
    }
    val over = (heard.size - HEARD_MOST).coerceAtLeast(0)
    repeat(over) { heard.removeAt(0) }
}

/** The radio songs worth adding after [seed]: playable, not the seed, and nothing the room has or has had. */
fun freshRadio(found: List<Track>, seed: String, room: Room, heard: List<String>): List<Track> {
    val seen = (room.queue + room.history).map { it.track.id }.toMutableSet()
    seen += heard
    seen += seed
    val fresh = found.filter { it.playable && seen.add(it.id) }
    return fresh.take(room.radioAllowance())
}

/**
 * What to tell this listener about song requests when the room goes from
 * [previous] to [room]: the leader about new ones, a guest about theirs
 * being sent and what became of them. [withdrawn] are requests this
 * listener cancelled, whose going is not news.
 */
fun requestNews(previous: Room, room: Room, me: String, withdrawn: MutableList<String>): List<String> {
    val news = mutableListOf<String>()
    val was = { id: String -> previous.requests.any { it.id == id } }
    val isStill = { id: String -> room.requests.any { it.id == id } }
    val arrived = room.requests.filter { !was(it.id) }
    val others = arrived.filter { it.by.id != me }
    if (room.answers(me)) {
        when (others.size) {
            0 -> {}
            1 -> news += "${others[0].by.name} requested “${others[0].track.title}”."
            else -> news += "${others.size} new song requests."
        }
    }
    when (val sent = arrived.size - others.size) {
        0 -> {}
        1 -> news += "Request sent. The leader decides what plays."
        else -> news += "$sent requests sent. The leader decides what plays."
    }
    // Accept all, or a change of mode, answers several at once: they are
    // told in one sentence.
    val added = mutableListOf<String>()
    val refused = mutableListOf<String>()
    for (request in previous.requests) {
        if (request.by.id != me || isStill(request.id)) continue
        if (withdrawn.remove(request.id)) continue
        val accepted = (room.queue + room.history).any { it.request == request.id }
        if (accepted) added += request.track.title else refused += request.track.title
    }
    requestOutcome(added, refused)?.let { news += it }
    return news
}

private fun requestOutcome(added: List<String>, refused: List<String>): String? = when {
    added.isEmpty() && refused.isEmpty() -> null
    refused.isEmpty() && added.size == 1 -> "“${added[0]}” was added to the queue."
    refused.isEmpty() -> "${added.size} of your requests were added to the queue."
    added.isEmpty() && refused.size == 1 -> "Your request for “${refused[0]}” wasn’t added."
    added.isEmpty() -> "${refused.size} of your requests weren’t added."
    else -> "${added.size} of your requests ${if (added.size == 1) "was" else "were"} added to the queue, " +
        "${refused.size} ${if (refused.size == 1) "wasn’t" else "weren’t"}."
}

/**
 * Whether the room was taken somewhere else in the same song between two
 * states: someone sought, which a player follows at once rather than as
 * drift it may wait to correct.
 */
fun jumped(previous: Room, room: Room): Boolean {
    if (previous.current != room.current) return false
    val run = if (previous.playing) (room.at - previous.at).coerceAtLeast(0.0) else 0.0
    return abs(previous.positionMs + run - room.positionMs) > 1000.0
}
