package dev.darksavci.spotified.together

import android.util.Log
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicLong
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.selects.onTimeout
import kotlinx.coroutines.selects.select
import kotlinx.coroutines.withTimeoutOrNull
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener

/*
 * The line to the relay: one coroutine that holds the socket, says hello,
 * takes a seat in a room, and then passes commands one way and the room the
 * other. A dropped line is picked up again with the seat's token. Mirrors
 * crates/app/src/together/client.rs.
 */

private const val TAG = "Together"

/** How long the line waits for the relay before it looks at its clocks. */
private const val POLL_MS = 120L

/** The relay must have said hello and ready within this. */
private const val HANDSHAKE_MS = 10_000L

/**
 * And must have made or found the room within this of being asked, unless
 * it said the leader is to let this listener in, which takes as long as the
 * leader takes.
 */
private const val ENTER_WITHIN_MS = 15_000L
private const val PING_EVERY_MS = 10_000L

/** How often a dropped line is tried again before giving up. */
private const val MOST_ATTEMPTS = 5

/** How long after the last step a seek is taken to have settled. */
internal const val SEEK_SETTLES_MS = 150L

/** How long a server is given to say hello when it is only being tested. */
private const val PROBE_MS = 8_000L

/** How to come into a room. */
sealed interface Enter {
    data class Create(val mode: Mode, val roomName: String) : Enter
    data class Join(val pin: String) : Enter
}

data class Options(
    val url: String,
    /** The name others see. */
    val name: String,
    /** The address of the picture others see; empty to share none. */
    val avatar: String,
    val enter: Enter,
)

/** What the line reports. */
sealed interface Event {
    object Reconnecting : Event

    /** The leader has yet to let this listener in. */
    object Waiting : Event

    /** A seat was taken; this is its member id. */
    data class Joined(val member: String) : Event

    /** The room as it now stands, and how far the relay's clock is ahead of this device's, in milliseconds. */
    data class RoomState(val room: Room, val offsetMs: Double) : Event

    /** A command was refused; the room goes on. */
    data class Refused(val why: String) : Event

    /** The room is over for this listener, with the relay's reason. */
    data class Ended(val reason: String) : Event

    /** The line could not be made, or was lost for good. */
    data class Failed(val why: String) : Event
}

private sealed interface Ask {
    /** A command's own fields; the envelope is added on the way out. */
    data class Command(val fields: JsonObject) : Ask
    data class Status(val status: String, val entry: String?) : Ask

    /** Leave the room, naming who leads it next if this listener did. */
    data class Leave(val next: String?) : Ask
}

/** What the socket says, as the line reads it. */
private sealed interface Heard {
    object Open : Heard
    data class Text(val text: String) : Heard
    data class Closed(val why: String) : Heard
}

/** The relay's clock is milliseconds since 1970, as JavaScript counts. */
fun nowMs(): Double = System.currentTimeMillis().toDouble()

/** A clock that only runs forward, for how long things take. */
internal fun monotonicMs(): Long = System.nanoTime() / 1_000_000

/**
 * A seek on its way to the room. Dragging the seek bar asks for one at
 * every step; only where it settles is sent, so a scrub neither trips the
 * relay's message limit nor races its own revisions. The song is pinned as
 * the seek is asked for: if the room has moved on by the time it settles,
 * the relay refuses it instead of applying it to the next song.
 */
internal class HeldSeek {
    private var held: Pair<JsonObject, Long>? = null

    /** Holds [command], in place of any held before it. */
    fun hold(command: JsonObject, current: String?, now: Long) {
        val pinned = if ("current" in command) command else JsonObject(command + ("current" to current.json()))
        held = pinned to now
    }

    /** The seek to send, once it has stood for long enough. */
    fun settled(now: Long): JsonObject? {
        val (command, since) = held ?: return null
        if (now - since < SEEK_SETTLES_MS) return null
        held = null
        return command
    }
}

private fun String?.json() = this?.let(::JsonPrimitive) ?: JsonNull

/** What is kept across a dropped line. */
internal data class Seat(
    /** The room's id and this seat's token, once seated. */
    val credential: Pair<String, String>? = null,
    val attempts: Int = 0,
)

/** How a spell on the line ended. */
private sealed interface Over {
    /** Left, ended or refused: nothing more to do. */
    object Done : Over

    /** The line dropped and may be picked up again. */
    data class Dropped(val why: String) : Over
}

class Connection(private val options: Options, client: OkHttpClient) {
    // A socket stays open for as long as the room does.
    private val client = client.newBuilder().readTimeout(0, TimeUnit.SECONDS).build()
    private val asks = Channel<Ask>(Channel.UNLIMITED)

    /** Connects in a coroutine of its own on [scope]. [deliver] is called from it. */
    fun start(scope: CoroutineScope, deliver: (Event) -> Unit) {
        scope.launch(Dispatchers.IO) { run(deliver) }
    }

    /** Sends a room command of [kind] with [fields] of its own. */
    fun command(kind: String, fields: JsonObject = JsonObject(emptyMap())) {
        asks.trySend(Ask.Command(JsonObject(fields + ("kind" to JsonPrimitive(kind)))))
    }

    /** Tells the room what this player is doing. */
    fun status(status: String, entry: String?) {
        asks.trySend(Ask.Status(status, entry))
    }

    /** Leaves the room, handing it to [next] if this listener led it. */
    fun leave(next: String? = null) {
        asks.trySend(Ask.Leave(next))
    }

    private suspend fun run(deliver: (Event) -> Unit) {
        var seat = Seat()
        while (true) {
            val (over, now) = spell(seat, deliver)
            seat = now
            when (over) {
                Over.Done -> return
                is Over.Dropped -> {
                    if (seat.credential == null || seat.attempts >= MOST_ATTEMPTS) {
                        Log.w(TAG, "listen together: ${over.why}")
                        deliver(Event.Failed("Disconnected from the room. Check the server and join again."))
                        return
                    }
                    deliver(Event.Reconnecting)
                    delay((500L shl seat.attempts).coerceAtMost(4000))
                    seat = seat.copy(attempts = seat.attempts + 1)
                }
            }
        }
    }

    /** One connection, from hello until it ends or drops; with the seat as it then is. */
    @OptIn(ExperimentalCoroutinesApi::class)
    private suspend fun spell(start: Seat, deliver: (Event) -> Unit): Pair<Over, Seat> {
        var seat = start
        val heard = Channel<Heard>(Channel.UNLIMITED)
        val socket = client.newWebSocket(Request.Builder().url(options.url).build(), listener(heard))
        val openedAt = monotonicMs()
        var open = false
        var ready = false
        var joined = false
        // When the room was asked for, and whether the leader is to be waited on.
        var askedAt: Long? = null
        var waiting = false
        var lastPing = monotonicMs()
        var offsetMs = 0.0
        // What a command is made against: the room as last heard.
        var revision = 0L
        var current: String? = null
        val seek = HeldSeek()
        fun send(message: JsonObject) {
            if (!socket.send(message.toString())) Log.d(TAG, "listen together: not sent")
        }
        fun done(over: Over): Pair<Over, Seat> = over to seat

        while (true) {
            val wake = select<Any?> {
                heard.onReceive { it }
                asks.onReceive { it }
                onTimeout(POLL_MS) { null }
            }
            when (wake) {
                Heard.Open -> open = true
                is Heard.Closed -> {
                    if (open || seat.credential != null) return done(Over.Dropped(wake.why))
                    Log.w(TAG, "listen together: ${wake.why}")
                    deliver(Event.Failed("Could not connect. Check the address, network, and server’s allowed origins."))
                    return done(Over.Done)
                }
                is Heard.Text -> {
                    val message = Incoming.read(wake.text)
                    if (message == null) {
                        Log.w(TAG, "listen together: a message could not be read")
                        continue
                    }
                    when (message) {
                        is Incoming.Hello -> {
                            if (message.version != VERSION) {
                                deliver(Event.Failed("This server is not compatible with Listen Together v2."))
                                socket.close(1000, null)
                                return done(Over.Done)
                            }
                            send(buildJsonObject { put("type", "hello"); put("version", VERSION) })
                        }
                        Incoming.Ready -> {
                            ready = true
                            send(ping())
                            send(entrance(options, seat))
                            askedAt = monotonicMs()
                        }
                        is Incoming.Pong -> offsetMs = message.at - (message.sent + nowMs()) / 2.0
                        is Incoming.Joined -> {
                            seat = Seat(message.roomId to message.token, 0)
                            joined = true
                            deliver(Event.Joined(message.member))
                        }
                        Incoming.Waiting -> {
                            waiting = true
                            deliver(Event.Waiting)
                        }
                        is Incoming.State -> {
                            revision = message.room.revision
                            current = message.room.current
                            deliver(Event.RoomState(message.room, offsetMs))
                        }
                        is Incoming.Error -> {
                            // A refusal before a seat is taken is the end of trying.
                            if (message.fatal || !joined) {
                                deliver(Event.Failed(message.message))
                                socket.close(1000, null)
                                return done(Over.Done)
                            }
                            deliver(Event.Refused(message.message))
                        }
                        is Incoming.Ended -> {
                            deliver(Event.Ended(message.reason))
                            socket.close(1000, null)
                            return done(Over.Done)
                        }
                        Incoming.Ack -> {}
                        Incoming.Unknown -> Log.d(TAG, "listen together: a message of a kind not known")
                    }
                }
                is Ask.Command -> {
                    if (wake.fields["kind"] == JsonPrimitive("seek")) seek.hold(wake.fields, current, monotonicMs())
                    else send(enveloped(wake.fields, revision, current))
                }
                is Ask.Status -> send(buildJsonObject {
                    put("type", "status"); put("status", wake.status); put("entry", wake.entry.json())
                })
                is Ask.Leave -> {
                    send(buildJsonObject { put("type", "leave"); put("next", wake.next.json()) })
                    socket.close(1000, null)
                    return done(Over.Done)
                }
            }
            val now = monotonicMs()
            if (!ready && now - openedAt > HANDSHAKE_MS) {
                deliver(Event.Failed("No v2 handshake received. Check the server address and upgrade the relay to v2."))
                socket.cancel()
                return done(Over.Done)
            }
            // A relay that took the hello but never answers the request for
            // a room would leave the app "connecting" for ever.
            val asked = askedAt
            if (!joined && !waiting && asked != null && now - asked > ENTER_WITHIN_MS) {
                Log.w(TAG, "listen together: the server did not answer the request for a room")
                deliver(Event.Failed("The server did not answer. Try again, or check that it is up to date."))
                socket.close(1000, null)
                return done(Over.Done)
            }
            if (joined && now - lastPing >= PING_EVERY_MS) {
                lastPing = now
                send(ping())
            }
            seek.settled(now)?.let { send(enveloped(it, revision, current)) }
        }
    }

    private fun listener(heard: Channel<Heard>) = object : WebSocketListener() {
        override fun onOpen(webSocket: WebSocket, response: Response) {
            heard.trySend(Heard.Open)
        }

        override fun onMessage(webSocket: WebSocket, text: String) {
            heard.trySend(Heard.Text(text))
        }

        override fun onClosing(webSocket: WebSocket, code: Int, reason: String) {
            webSocket.close(1000, null)
            heard.trySend(Heard.Closed("the server closed the line"))
        }

        override fun onFailure(webSocket: WebSocket, t: Throwable, response: Response?) {
            heard.trySend(Heard.Closed(t.message ?: "the line failed"))
        }
    }

    companion object {
        /**
         * Sees whether [url] answers as a version 2 relay does, without
         * making or joining a room.
         */
        suspend fun probe(client: OkHttpClient, url: String): Result<Unit> {
            val heard = Channel<Heard>(Channel.UNLIMITED)
            val socket = client.newWebSocket(Request.Builder().url(url).build(), object : WebSocketListener() {
                override fun onMessage(webSocket: WebSocket, text: String) {
                    heard.trySend(Heard.Text(text))
                }

                override fun onClosing(webSocket: WebSocket, code: Int, reason: String) {
                    heard.trySend(Heard.Closed("closed"))
                }

                override fun onFailure(webSocket: WebSocket, t: Throwable, response: Response?) {
                    heard.trySend(Heard.Closed(t.message ?: "failed"))
                }
            })
            val first = withTimeoutOrNull(PROBE_MS) { heard.receive() }
            socket.close(1000, null)
            return when (first) {
                null -> Result.failure(IllegalStateException("No v2 handshake. Check the address and the relay version."))
                is Heard.Text -> greeting(first.text)
                else -> {
                    Log.d(TAG, "listen together: the test could not connect: $first")
                    Result.failure(IllegalStateException("Could not connect. Check the address, network and allowed origins."))
                }
            }
        }
    }
}

private fun ping() = buildJsonObject { put("type", "ping"); put("sent", nowMs()) }

/** A command as the relay takes it: numbered, and made against the room as last heard. */
internal fun enveloped(command: JsonObject, revision: Long, current: String?): JsonObject {
    val fields = command.toMutableMap()
    fields["op"] = JsonPrimitive(operationId())
    fields["base"] = JsonPrimitive(revision)
    if ("current" !in fields) fields["current"] = current.json()
    return buildJsonObject { put("type", "command"); put("command", JsonObject(fields)) }
}

/** How this spell comes into the room: back to its seat if it has one. */
internal fun entrance(options: Options, seat: Seat): JsonObject {
    seat.credential?.let { (roomId, token) ->
        return buildJsonObject { put("type", "resume"); put("roomId", roomId); put("token", token) }
    }
    val profile = buildJsonObject { put("name", options.name); put("avatar", options.avatar) }
    return when (val enter = options.enter) {
        is Enter.Create -> buildJsonObject {
            put("type", "create"); put("mode", enter.mode.wire); put("roomName", enter.roomName); put("profile", profile)
        }
        is Enter.Join -> buildJsonObject { put("type", "join"); put("pin", enter.pin); put("profile", profile) }
    }
}

/** What a server's first message says of it. */
internal fun greeting(text: String): Result<Unit> {
    fun no(why: String) = Result.failure<Unit>(IllegalStateException(why))
    return when (val message = Incoming.read(text)) {
        is Incoming.Hello -> if (message.version == VERSION) Result.success(Unit) else no("This relay does not support Listen Together v2.")
        is Incoming.Error -> no(message.message.ifEmpty { "Connection rejected." })
        else -> no("The server returned an invalid response.")
    }
}

private val operations = AtomicLong(0)

/** A name for a command that no other command of this run shares; the relay uses it to tell a retry from a new command. */
internal fun operationId(): String {
    val count = operations.getAndIncrement()
    return "${nowMs().toLong().toString(16)}-${count.toString(16)}"
}
