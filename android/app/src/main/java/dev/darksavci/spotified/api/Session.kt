package dev.darksavci.spotified.api

import java.io.IOException
import java.util.concurrent.TimeUnit
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.flow.flowOn
import kotlinx.coroutines.isActive
import kotlinx.serialization.Serializable
import kotlinx.serialization.builtins.ListSerializer
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonArray
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put
import kotlinx.serialization.json.putJsonObject
import okhttp3.HttpUrl
import okhttp3.OkHttpClient
import okhttp3.Request

/*
 * The playback session: commands and engine reports sent to the core, and the
 * projections it sends back. Mirrors crates/client/src/session.rs, including
 * the wire's oddity that Command and EngineEvent travel with Go's field names.
 */

/** A command to the core's session. */
sealed interface Command {
    data class Play(val tracks: List<Track>, val startIndex: Int, val origin: String, val paused: Boolean = false) : Command
    object Toggle : Command
    object Next : Command
    object Previous : Command
    data class Seek(val positionMs: Long) : Command
    data class SetRepeat(val repeat: Repeat) : Command
    data class SetShuffle(val on: Boolean) : Command
    data class SetVolume(val volume: Float) : Command
    data class Jump(val index: Int) : Command
    data class Remove(val index: Int) : Command
    data class Move(val from: Int, val to: Int) : Command
    /** Adds tracks at a place, or at the end when [at] is null. */
    data class Enqueue(val tracks: List<Track>, val at: Int?) : Command
    data class FollowRoom(val tracks: List<Track>, val index: Int, val entry: String, val positionMs: Long, val playing: Boolean) : Command
    data class LeaveRoom(val keepQueue: Boolean, val thenOwnQueue: Boolean) : Command

    fun wire(): JsonObject = buildJsonObject {
        fun tracks(name: String, list: List<Track>) =
            put(name, CoreJson.encodeToJsonElement(ListSerializer(Track.serializer()), list))
        when (val c = this@Command) {
            is Play -> {
                put("Kind", "play"); tracks("Tracks", c.tracks)
                put("StartIndex", c.startIndex); put("Origin", c.origin)
                if (c.paused) put("Paused", true)
            }
            Toggle -> put("Kind", "toggle")
            Next -> put("Kind", "next")
            Previous -> put("Kind", "prev")
            is Seek -> { put("Kind", "seek"); put("PositionMs", c.positionMs) }
            is SetRepeat -> { put("Kind", "set_repeat"); put("Repeat", c.repeat.name.lowercase()) }
            is SetShuffle -> { put("Kind", "set_shuffle"); put("Shuffle", c.on) }
            is SetVolume -> { put("Kind", "set_volume"); put("Volume", c.volume) }
            is Jump -> { put("Kind", "jump"); put("At", c.index) }
            is Remove -> { put("Kind", "remove"); put("At", c.index) }
            is Move -> { put("Kind", "move"); put("From", c.from); put("To", c.to) }
            // The core reads a place outside the queue as "at the end".
            is Enqueue -> { put("Kind", "enqueue"); tracks("Insert", c.tracks); put("At", c.at ?: -1) }
            is FollowRoom -> {
                put("Kind", "follow_room"); tracks("Tracks", c.tracks); put("StartIndex", c.index)
                put("ExpectedID", c.entry); put("PositionMs", c.positionMs); put("Playing", c.playing)
            }
            is LeaveRoom -> { put("Kind", "leave_room"); put("KeepQueue", c.keepQueue); put("ThenOwnQueue", c.thenOwnQueue) }
        }
    }
}

enum class EngineEventKind(val wire: String) {
    Loaded("loaded"), Position("position"), Ended("ended"), Failed("failed"), Stalled("stalled"), Blocked("blocked")
}

/** What the engine reports; [reason] is one the core knows: empty, "network" or "stalled". */
data class EngineEvent(
    val kind: EngineEventKind,
    val epoch: Long,
    val positionMs: Long = 0,
    val durationMs: Long = 0,
    val reason: String = "",
)

/** The playback settings that are the core's to act on. Every field is sent every time. */
data class CoreSettings(
    val crossfadeMs: Long = 0,
    val gapless: Boolean = true,
    val resumeOnLaunch: Boolean = true,
    val reportToYouTube: Boolean = false,
    val cacheMaxMb: Long = 2048,
    val autoplay: Boolean = true,
    val blockedTracks: List<String> = emptyList(),
    val blockedArtists: List<String> = emptyList(),
)

@Serializable
private data class Answer(val projection: Projection = Projection(), val rejected: String = "")

class Session(private val api: Api, private val streaming: OkHttpClient) {

    /** Announces this device; needed again after the event stream drops, which unregisters it. */
    suspend fun register(deviceId: String): Projection =
        api.call(Answer.serializer(), "POST", buildJsonObject {
            put("deviceId", deviceId)
            put("name", "Youtube Music Spotified (Android)")
            putJsonObject("capabilities") {
                put("EQ", true)
                put("Crossfade", "true")
                put("Normalization", true)
                put("PreciseSeek", true)
                put("VolumeSteps", 0)
            }
        }, "session", "register").projection

    /** Sends a command; a refusal is an error carrying the core's reason. */
    suspend fun command(deviceId: String, command: Command): Projection {
        val answer = api.call(Answer.serializer(), "POST", buildJsonObject {
            put("deviceId", deviceId)
            put("command", command.wire())
        }, "session", "command")
        if (answer.rejected.isNotEmpty()) throw ApiError.Status(200, "The player refused that (${answer.rejected}).")
        return answer.projection
    }

    suspend fun engineEvent(deviceId: String, event: EngineEvent) =
        api.post("session", "engine-event", body = buildJsonObject {
            put("deviceId", deviceId)
            putJsonObject("event") {
                put("Kind", event.kind.wire)
                put("Epoch", event.epoch)
                put("PositionMs", event.positionMs)
                put("DurationMs", event.durationMs)
                put("Reason", event.reason)
            }
        })

    /** Plays a song and has the core carry on from it with songs like it. */
    suspend fun startRadio(deviceId: String, track: Track) =
        radio(buildJsonObject {
            put("deviceId", deviceId)
            put("track", CoreJson.encodeToJsonElement(Track.serializer(), track))
        })

    /** Plays one of YouTube's own queues: an artist's mix, or a shuffle of their songs. */
    suspend fun startMix(deviceId: String, playlistId: String, videoId: String, params: String, origin: String) =
        radio(buildJsonObject {
            put("deviceId", deviceId); put("playlistId", playlistId); put("videoId", videoId)
            put("params", params); put("origin", origin)
        })

    private suspend fun radio(body: JsonElement) {
        val answer = api.call(Answer.serializer(), "POST", body, "session", "radio")
        if (answer.rejected.isNotEmpty()) throw ApiError.Status(200, "The player refused that (${answer.rejected}).")
    }

    suspend fun setSettings(s: CoreSettings) =
        api.post("session", "settings", body = buildJsonObject {
            put("crossfadeMs", s.crossfadeMs)
            put("gapless", s.gapless)
            put("resumeOnLaunch", s.resumeOnLaunch)
            put("reportToYouTube", s.reportToYouTube)
            put("cacheMaxMB", s.cacheMaxMb)
            put("autoplay", s.autoplay)
            putJsonObject("blocked") {
                put("tracks", buildJsonArray { s.blockedTracks.forEach { add(JsonPrimitive(it)) } })
                put("artists", buildJsonArray { s.blockedArtists.forEach { add(JsonPrimitive(it)) } })
                put("albums", buildJsonArray { })
            }
        })

    /**
     * The session's projections, for as long as the stream lasts: a server-sent
     * event stream of `data:` lines. Ends when the core closes it, or throws
     * when it cannot be reached; the caller registers again and reconnects.
     */
    fun events(deviceId: String, origin: suspend () -> HttpUrl): Flow<Projection> = flow {
        val url = origin().newBuilder().addPathSegments("v1/session/events")
            .addQueryParameter("deviceId", deviceId).build()
        val call = streaming.newCall(Request.Builder().url(url).build())
        val response = try {
            call.execute()
        } catch (e: IOException) {
            throw ApiError.Unreachable(e.message.orEmpty())
        }
        response.use {
            if (!it.isSuccessful) throw Api.errorFor(it.code, "")
            val source = it.body.source()
            val data = StringBuilder()
            while (currentCoroutineContext().isActive) {
                val line = source.readUtf8Line() ?: break
                when {
                    line.startsWith("data:") -> data.append(line.removePrefix("data:").trimStart())
                    line.isEmpty() && data.isNotEmpty() -> {
                        runCatching { CoreJson.decodeFromString(Projection.serializer(), data.toString()) }
                            .onSuccess { projection -> emit(projection) }
                        data.clear()
                    }
                }
            }
        }
    }.flowOn(Dispatchers.IO)

    companion object {
        /** The event stream stays open; the core keeps it alive every 25 s. */
        fun streamingClient(base: OkHttpClient): OkHttpClient =
            base.newBuilder().readTimeout(60, TimeUnit.SECONDS).callTimeout(0, TimeUnit.SECONDS).build()
    }
}
