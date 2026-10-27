package dev.darksavci.spotified.together

import dev.darksavci.spotified.api.ArtistRef
import dev.darksavci.spotified.api.Artwork
import dev.darksavci.spotified.api.Track
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonArray
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.doubleOrNull
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.longOrNull
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.put

/*
 * What is said over the socket, in the relay's version 2. Mirrors
 * crates/app/src/together/protocol.rs.
 *
 * The relay owns the room: its queue, what is playing, and where in the
 * song everyone should be. Members send commands and hear the whole room
 * back after every change. Unknown fields are ignored, so a newer relay
 * does not break an older app.
 */

/** The protocol this app speaks. */
const val VERSION = 2L

/** The most songs a single command may carry. */
const val MOST_TRACKS = 100

internal val RelayJson = Json {
    ignoreUnknownKeys = true
    coerceInputValues = true
    explicitNulls = false
    encodeDefaults = true
}

/** Who may steer a room. */
@Serializable
enum class Mode(val wire: String, val title: String, val about: String) {
    /** Everyone can play, pause, skip and shape the queue. */
    @SerialName("collaborative")
    Collaborative("collaborative", "Everyone’s the DJ", "Everyone can play, pause, skip and shape the queue."),

    /** Friends request songs; the leader decides what plays. */
    @SerialName("contributions")
    Contributions("contributions", "Take requests", "Friends request songs. You approve what plays and when."),

    /** The leader handles the music; friends listen. */
    @SerialName("listen")
    Listen("listen", "Just listen", "You handle the music. Friends settle in and listen.");

    companion object {
        fun ofWire(wire: String): Mode = entries.firstOrNull { it.wire == wire } ?: Collaborative
    }
}

/** How a room's queue repeats. */
@Serializable
enum class Repeat(val wire: String) {
    @SerialName("off") Off("off"),
    @SerialName("one") One("one"),
    @SerialName("all") All("all");

    /** The mode after this one, as the player's button steps: off, all, one. */
    fun next(): Repeat = when (this) {
        Off -> All
        All -> One
        One -> Off
    }
}

/** The order additions take in the queue. */
@Serializable
enum class Policy(val wire: String, val title: String) {
    /** As they came. */
    @SerialName("fifo") Fifo("fifo", "First in, first out"),

    /** One from each person in turn. */
    @SerialName("turns") Turns("turns", "Take turns"),
}

/** A picture's address as the relay passes it on, as the artwork it is drawn from; nothing for someone who shares none. */
fun pictureOf(url: String): List<Artwork> =
    if (url.isEmpty()) emptyList() else listOf(Artwork(url, 96, 96))

@Serializable
data class Member(
    val id: String = "",
    val name: String = "",
    /** The address of their picture; empty for someone who shares none. */
    val avatar: String = "",
    /** Has answered the ready check that is on. */
    val ready: Boolean = false,
    val connected: Boolean = false,
    /** What their player is doing: `listening`, `paused`, `buffering`… */
    val status: String = "",
    /** `dj` or `listener`; the leader is named by the room, not by a role. */
    val role: String = "",
) {
    val picture: List<Artwork> get() = pictureOf(avatar)
}

/** Someone in the room, as the relay names them beside what they did. */
@Serializable
data class Person(val id: String = "", val name: String = "", val avatar: String = "") {
    val picture: List<Artwork> get() = pictureOf(avatar)
}

@Serializable
data class RoomArtist(val name: String = "")

/** A song as the relay passes it on: only what it lets through. */
@Serializable
data class RoomTrack(
    val id: String = "",
    val title: String = "",
    val artists: List<RoomArtist> = emptyList(),
    val durationMs: Double = 0.0,
    val artwork: List<Artwork> = emptyList(),
) {
    /** Whose song it is, as a row writes it. */
    val artistNames: String get() = artists.joinToString(", ") { it.name }

    fun toTrack() = Track(
        id = id,
        title = title,
        artists = artists.map { ArtistRef(name = it.name) },
        durationMs = durationMs.coerceAtLeast(0.0).toLong(),
        artwork = artwork,
        playable = true,
    )
}

@Serializable
data class Entry(
    val id: String = "",
    val track: RoomTrack = RoomTrack(),
    val addedBy: Person = Person(),
    /** When it was added, on the relay's clock. */
    val addedAt: Double = 0.0,
    /** Added by the room's radio, not chosen by anyone. */
    val radio: Boolean = false,
    /** The request this came in as, if it was asked for. */
    val request: String? = null,
)

/** A line of the room's activity. */
@Serializable
data class Happening(val id: String = "", val text: String = "", val at: Double = 0.0)

/** Someone waiting for the leader to let them in. */
@Serializable
data class Knock(val id: String = "", val name: String = "", val avatar: String = "") {
    val picture: List<Artwork> get() = pictureOf(avatar)
}

/** A ready check. Once everyone has answered, or the leader says so, it has a moment at which the room starts. */
@Serializable
data class Countdown(val expires: Double = 0.0, val startAt: Double? = null)

/** A song a guest asked for. */
@Serializable
data class SongRequest(val id: String = "", val track: RoomTrack = RoomTrack(), val by: Person = Person())

/**
 * An edit of the queue that can be taken back: by whom, until when, and
 * only while the room is still at the revision the edit left it at.
 */
@Serializable
data class Undo(val revision: Long = 0, val expires: Double = 0.0, val by: String = "")

/** A change to whether the room's video is shown: by whom, and which change of the room it was. */
@Serializable
data class RoomVideo(val shown: Boolean = false, val by: String = "", val revision: Long = 0)

@Serializable
data class Room(
    val name: String = "",
    val id: String = "",
    val pin: String = "",
    val members: List<Member> = emptyList(),
    /** The leader's member id. */
    val owner: String = "",
    val mode: Mode = Mode.Collaborative,
    val queue: List<Entry> = emptyList(),
    /** The id of the entry that is playing. */
    val current: String? = null,
    /** Where the song was at [at], on the relay's clock, in milliseconds. */
    val positionMs: Double = 0.0,
    val at: Double = 0.0,
    val playing: Boolean = false,
    /** Counts changes; a command names the one it was made against. */
    val revision: Long = 0,
    /** When the room closes, on the relay's clock. */
    val expires: Double = 0.0,
    /** The queue has run out. */
    val finished: Boolean = false,
    /** Songs that have played, oldest first. */
    val history: List<Entry> = emptyList(),
    /** What has happened in the room lately, oldest first. */
    val activity: List<Happening> = emptyList(),
    val repeat: Repeat = Repeat.Off,
    val policy: Policy = Policy.Fifo,
    /** Whether a song may wait in the queue twice. */
    val duplicates: Boolean = false,
    /** How many songs a guest may have waiting. */
    val limit: Int = 0,
    /** No one new may join. */
    val locked: Boolean = false,
    /** Listeners may vote the song away. */
    val voteSkip: Boolean = false,
    /** The members who have voted to skip this song. */
    val votes: List<String> = emptyList(),
    /** The leader lets each new listener in. */
    val joinApproval: Boolean = false,
    /** Those waiting to be let in. */
    val pending: List<Knock> = emptyList(),
    /** A ready check, or the count to a shared start after one. */
    val countdown: Countdown? = null,
    /** Songs guests asked for, waiting for the leader or a DJ. */
    val requests: List<SongRequest> = emptyList(),
    /** Requests go straight into the queue. */
    val autoAccept: Boolean = false,
    /** The last edit of the queue, while it can still be taken back. */
    val undo: Undo? = null,
    val lastControlledBy: Person? = null,
    /** The last time someone who steers showed or hid the video. */
    val video: RoomVideo? = null,
) {
    /** The entry that is playing and its place in the queue. */
    fun currentEntry(): IndexedValue<Entry>? {
        val id = current ?: return null
        val index = queue.indexOfFirst { it.id == id }
        return if (index < 0) null else IndexedValue(index, queue[index])
    }

    /** Where the song is at [serverNow], as the relay reckons it. */
    fun positionAt(serverNow: Double): Long {
        val elapsed = if (playing) (serverNow - at).coerceAtLeast(0.0) else 0.0
        val length = currentEntry()?.value?.track?.durationMs?.takeIf { it > 0.0 } ?: 86_400_000.0
        return (positionMs + elapsed).coerceIn(0.0, length).toLong()
    }

    fun leader(): Member? = members.firstOrNull { it.id == owner }

    /** Whether [member] may steer: the leader, a DJ, or anyone in a room where everyone is. */
    fun mayControl(member: String): Boolean =
        mode == Mode.Collaborative || owner == member || members.any { it.id == member && it.role == "dj" }

    fun member(id: String): Member? = members.firstOrNull { it.id == id }

    /** What the room is called: its name, or whose it is. */
    fun title(): String {
        if (name.isNotEmpty()) return name
        return "${leader()?.name ?: "Your friends"}’s room"
    }

    /** The room's queue as tracks, for the player to mirror. */
    fun tracks(): List<Track> = queue.map { it.track.toTrack() }
}

/** A song as the relay takes it. It keeps only these fields and refuses a track whose id is not a video id. */
fun trackJson(track: Track): JsonObject = buildJsonObject {
    put("id", track.id)
    put("title", track.title.take(300))
    put("durationMs", track.durationMs)
    put("isVideo", track.isVideo)
    put("explicit", track.explicit)
    put("artists", buildJsonArray { track.artists.take(10).forEach { add(buildJsonObject { put("name", it.name) }) } })
    put("artwork", buildJsonArray {
        track.artwork.take(3).forEach {
            add(buildJsonObject { put("url", it.url); put("width", it.width); put("height", it.height) })
        }
    })
}

fun tracksJson(tracks: List<Track>): JsonArray = JsonArray(tracks.take(MOST_TRACKS).map(::trackJson))

/** What the relay says. */
sealed interface Incoming {
    data class Hello(val version: Long) : Incoming
    object Ready : Incoming

    /** The answer to a ping: when it was sent, and the relay's clock then. */
    data class Pong(val sent: Double, val at: Double) : Incoming

    /** A seat in the room, and what brings it back after a dropped line. */
    data class Joined(val member: String, val token: String, val roomId: String) : Incoming

    /** The leader has yet to let this listener in. */
    object Waiting : Incoming
    data class State(val room: Room) : Incoming
    object Ack : Incoming

    /** [op] is the command this refuses, when it refuses one. */
    data class Error(val message: String, val op: String?, val fatal: Boolean) : Incoming

    /** The room is over for this listener. */
    data class Ended(val reason: String) : Incoming
    object Unknown : Incoming

    companion object {
        /** Reads a message; null for one that cannot be read at all. */
        fun read(text: String): Incoming? = runCatching {
            val o = RelayJson.parseToJsonElement(text).jsonObject
            fun string(name: String) = (o[name] as? JsonPrimitive)?.contentOrNull
            fun number(name: String) = (o[name] as? JsonPrimitive)?.doubleOrNull ?: 0.0
            when (string("type")) {
                "hello" -> Hello((o["version"] as? JsonPrimitive)?.longOrNull ?: 0)
                "ready" -> Ready
                "pong" -> Pong(number("sent"), number("at"))
                "joined" -> Joined(
                    string("member") ?: return null,
                    string("token") ?: return null,
                    string("roomId") ?: return null,
                )
                "waiting" -> Waiting
                "state" -> State(RelayJson.decodeFromJsonElement(Room.serializer(), o["room"] ?: return null))
                "ack" -> Ack
                "error" -> Error(
                    string("message").orEmpty(),
                    string("op"),
                    (o["fatal"] as? JsonPrimitive)?.booleanOrNull ?: false,
                )
                "ended" -> Ended(string("reason").orEmpty())
                else -> Unknown
            }
        }.getOrNull()
    }
}

private const val NO_ADDRESS = "Enter the server's address, starting with wss://"

/**
 * Checks a relay's address as the relay's own client does: `wss://`, or
 * `ws://` to this device for testing, and nothing but host and path.
 */
fun checkedAddress(input: String): Result<String> {
    val address = input.trim()
    val split = address.indexOf("://")
    if (split < 0) return refused(NO_ADDRESS)
    val scheme = address.substring(0, split)
    val rest = address.substring(split + 3)
    val host = rest.split('/', ':').first()
    if (host.isEmpty()) return refused(NO_ADDRESS)
    val local = host in listOf("localhost", "127.0.0.1", "[::1]")
    if (!(scheme == "wss" || (scheme == "ws" && local))) {
        return refused("Use wss:// for a remote server, or ws://localhost for local testing.")
    }
    if (rest.any { it == '@' || it == '?' || it == '#' }) {
        return refused("The address must not contain a password, a query or a fragment.")
    }
    return Result.success(address)
}

private fun refused(why: String): Result<String> = Result.failure(IllegalArgumentException(why))
