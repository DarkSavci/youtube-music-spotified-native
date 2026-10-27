package dev.darksavci.spotified.data

import android.content.Context
import android.content.Intent
import android.net.Uri
import dev.darksavci.spotified.api.Projection
import dev.darksavci.spotified.api.Track
import dev.darksavci.spotified.playback.Playback
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import okhttp3.FormBody
import okhttp3.HttpUrl.Companion.toHttpUrl
import okhttp3.OkHttpClient
import okhttp3.Request
import java.io.File
import java.security.MessageDigest

/**
 * Last.fm: signing in through its web page, "now playing" when a song starts
 * and a scrobble once half of it or four minutes have been heard, queued on
 * disk while offline.
 *
 * Last.fm gives each app a key and a secret of its own; this one asks the
 * listener for theirs (last.fm/api/account/create) rather than shipping one.
 */
class LastFm(
    private val context: Context,
    private val http: OkHttpClient,
    private val settings: SettingsStore,
    private val scope: CoroutineScope,
) {
    private val queue = File(context.filesDir, "scrobbles.txt")

    /** Follows what plays, and tells Last.fm when it is connected. */
    fun follow(playback: Playback) {
        scope.launch {
            var current: Track? = null
            var startedAt = 0L
            var heardMs = 0L
            var lastPosition = 0L
            var sent = false
            playback.projection.collect { p: Projection ->
                val track = p.state.current
                if (track?.id != current?.id) {
                    current = track
                    startedAt = System.currentTimeMillis() / 1000
                    heardMs = 0
                    lastPosition = p.state.positionMs
                    sent = false
                    if (track != null && p.target.playing) nowPlaying(track)
                    return@collect
                }
                val position = p.state.positionMs
                if (position > lastPosition && position - lastPosition < 5_000) heardMs += position - lastPosition
                lastPosition = position
                val t = track ?: return@collect
                val enough = if (t.durationMs > 30_000) minOf(t.durationMs / 2, 240_000) else Long.MAX_VALUE
                if (!sent && heardMs >= enough) {
                    sent = true
                    scrobble(t, startedAt)
                }
            }
        }
        scope.launch {
            while (true) {
                delay(15 * 60_000)
                flush()
            }
        }
    }

    private suspend fun nowPlaying(track: Track) {
        val s = settings.current()
        if (s.lastfmSession.isEmpty()) return
        runCatching { call(s, "track.updateNowPlaying", mapOf("artist" to firstArtist(track), "track" to track.title)) }
    }

    private suspend fun scrobble(track: Track, at: Long) {
        val s = settings.current()
        if (s.lastfmSession.isEmpty()) return
        val line = listOf(firstArtist(track), track.title, track.album?.name.orEmpty(), at.toString()).joinToString("\t")
        withContext(Dispatchers.IO) { synchronized(queue) { queue.appendText(line.replace("\n", " ") + "\n") } }
        flush()
    }

    /** Sends what was scrobbled while Last.fm could not be reached, fifty at a time. */
    private suspend fun flush() = withContext(Dispatchers.IO) {
        val s = settings.current()
        if (s.lastfmSession.isEmpty() || !queue.exists()) return@withContext
        val lines = synchronized(queue) { queue.readLines().filter { it.isNotBlank() } }
        val sentCount = lines.chunked(50).takeWhile { batch ->
            val params = mutableMapOf<String, String>()
            batch.forEachIndexed { i, line ->
                val (artist, title, album, ts) = line.split("\t").let { it + List(4 - it.size) { "" } }
                params["artist[$i]"] = artist
                params["track[$i]"] = title
                params["timestamp[$i]"] = ts
                if (album.isNotEmpty()) params["album[$i]"] = album
            }
            runCatching { call(s, "track.scrobble", params) }.isSuccess
        }.sumOf { it.size }
        if (sentCount > 0) synchronized(queue) {
            val rest = queue.readLines().filter { it.isNotBlank() }.drop(sentCount)
            queue.writeText(rest.joinToString("") { "$it\n" })
        }
    }

    private fun firstArtist(track: Track) = track.artists.firstOrNull()?.name ?: track.artistNames

    /** A signed call: the parameters sorted and hashed with the secret, as Last.fm asks. */
    private suspend fun call(s: Settings, method: String, params: Map<String, String>): JsonObject = withContext(Dispatchers.IO) {
        val all = params + mapOf("method" to method, "api_key" to s.lastfmKey, "sk" to s.lastfmSession)
        val body = FormBody.Builder().apply {
            all.forEach { (k, v) -> add(k, v) }
            add("api_sig", signature(all, s.lastfmSecret))
            add("format", "json")
        }.build()
        val request = Request.Builder().url(API).post(body).build()
        http.newCall(request).execute().use { response ->
            val json = Json.parseToJsonElement(response.body.string()) as JsonObject
            if (!response.isSuccessful || "error" in json) error((json["message"] as? JsonPrimitive)?.content ?: "Last.fm said no")
            json
        }
    }

    /** Finishes signing in once the listener has allowed the app on Last.fm's page. */
    suspend fun finish(): String = withContext(Dispatchers.IO) {
        val s = settings.current()
        val token = s.lastfmToken.ifEmpty { error("Start by connecting Last.fm") }
        val params = mapOf("method" to "auth.getSession", "api_key" to s.lastfmKey, "token" to token)
        val url = API.toHttpUrl().newBuilder().apply {
            params.forEach { (k, v) -> addQueryParameter(k, v) }
            addQueryParameter("api_sig", signature(params, s.lastfmSecret))
            addQueryParameter("format", "json")
        }.build()
        val json = http.newCall(Request.Builder().url(url).build()).execute().use { Json.parseToJsonElement(it.body.string()) as JsonObject }
        val session = json["session"] as? JsonObject ?: error((json["message"] as? JsonPrimitive)?.content ?: "Last.fm did not let the app in")
        val key = (session["key"] as JsonPrimitive).content
        val name = (session["name"] as JsonPrimitive).content
        settings.update { it.copy(lastfmSession = key, lastfmUser = name, lastfmToken = "") }
        name
    }

    /** Asks Last.fm for a token and opens its page to allow the app. */
    suspend fun start(key: String, secret: String) {
        settings.update { it.copy(lastfmKey = key.trim(), lastfmSecret = secret.trim()) }
        val params = mapOf("method" to "auth.getToken", "api_key" to key.trim())
        val url = API.toHttpUrl().newBuilder().apply {
            params.forEach { (k, v) -> addQueryParameter(k, v) }
            addQueryParameter("api_sig", signature(params, secret.trim()))
            addQueryParameter("format", "json")
        }.build()
        val token = withContext(Dispatchers.IO) {
            http.newCall(Request.Builder().url(url).build()).execute().use {
                ((Json.parseToJsonElement(it.body.string()) as JsonObject)["token"] as? JsonPrimitive)?.content
            }
        } ?: error("Last.fm did not accept that key")
        settings.update { it.copy(lastfmToken = token) }
        val page = "https://www.last.fm/api/auth/?api_key=${Uri.encode(key.trim())}&token=${Uri.encode(token)}"
        context.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(page)).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
    }

    suspend fun connected() = settings.settings.first().lastfmSession.isNotEmpty()

    companion object {
        private const val API = "https://ws.audioscrobbler.com/2.0/"

        fun signature(params: Map<String, String>, secret: String): String {
            val text = params.toSortedMap().entries.joinToString("") { it.key + it.value } + secret
            return MessageDigest.getInstance("MD5").digest(text.toByteArray()).joinToString("") { "%02x".format(it) }
        }

        /** Where to make a key and secret of one's own. */
        fun authorise(context: Context) {
            context.startActivity(
                Intent(Intent.ACTION_VIEW, Uri.parse("https://www.last.fm/api/account/create")).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK),
            )
        }
    }
}
