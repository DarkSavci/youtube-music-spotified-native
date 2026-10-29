package dev.darksavci.spotified.api

import java.io.IOException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.serialization.KSerializer
import kotlinx.serialization.Serializable
import kotlinx.serialization.builtins.ListSerializer
import kotlinx.serialization.builtins.nullable
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put
import kotlinx.serialization.json.putJsonArray
import kotlinx.serialization.serializer
import okhttp3.HttpUrl
import okhttp3.HttpUrl.Companion.toHttpUrl
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody

/** Why a call to the core failed, with a sentence fit to show. */
sealed class ApiError(message: String) : Exception(message) {
    class Unreachable(cause: String) : ApiError("The playback service is not answering.") {
        val detail = cause
    }
    object SignedOut : ApiError("Sign in to see this.")
    object RateLimited : ApiError("YouTube Music is limiting requests. Try again in a little while.")
    class Status(val code: Int, message: String) :
        ApiError(message.ifEmpty { "Something went wrong (error $code)." })
    class Decode(cause: String) : ApiError("The answer could not be read.") {
        val detail = cause
    }
}

@Serializable
private data class Me(val account: Account? = null)

@Serializable
private data class Created(val id: String = "")

@Serializable
private data class Reloaded(val signedIn: Boolean = false)

@Serializable
data class Health(val rateLimited: Boolean = false, val offline: Boolean = false, val error: String = "")

@Serializable
private data class Loudness(val loudnessLkfs: Double? = null)

@Serializable
private data class ErrorBody(val error: String = "", val reauth: Boolean = false, val rateLimited: Boolean = false)

/**
 * Talks to the core over HTTP on the loopback address. Mirrors
 * crates/client/src/lib.rs. Every call is a suspend function run on the IO
 * dispatcher; the core answers from cache in milliseconds, and a miss waits
 * on YouTube behind its rate governor.
 *
 * [origin] is read per call: the core's address and token change when it
 * restarts, as it does after a sign-in.
 */
class Api(private val http: OkHttpClient, private val origin: suspend () -> HttpUrl) {

    suspend fun home(mood: String = ""): BrowsePage =
        get(BrowsePage.serializer(), "home") { if (mood.isNotEmpty()) addQueryParameter("mood", mood) }

    suspend fun homeMore(continuation: String): BrowsePage =
        get(BrowsePage.serializer(), "home") { addQueryParameter("continuation", continuation) }

    /** Home, with its next [pages] - 1 pages of shelves joined on: the first alone holds only a couple. */
    suspend fun homeWithMore(mood: String = "", pages: Int = 4): BrowsePage {
        val first = home(mood)
        val shelves = first.shelves.toMutableList()
        var next = first.continuation
        for (i in 1 until pages) {
            if (next.isEmpty()) break
            val page = runCatching { homeMore(next) }.getOrNull() ?: break
            shelves += page.shelves
            next = page.continuation
        }
        return first.copy(shelves = shelves, continuation = next)
    }

    /** Home's first [pages] pages of shelves: the first alone holds only a couple. */
    suspend fun homeShelves(pages: Int = 3): List<Shelf> {
        val first = home()
        val shelves = first.shelves.toMutableList()
        var page: BrowsePage? = first
        for (i in 1 until pages) {
            val next = page?.continuation.orEmpty().ifEmpty { break }
            page = runCatching { homeMore(next) }.getOrNull()
            shelves += page?.shelves.orEmpty()
        }
        return shelves
    }

    suspend fun browse(id: String, params: String = ""): BrowsePage =
        get(BrowsePage.serializer(), "browse", id) { addQueryParameter("params", params) }

    suspend fun history(): List<Track> = getList(Track.serializer(), "me", "history")

    suspend fun suggest(query: String): List<String> =
        getList(serializer<String>(), "suggest") { addQueryParameter("q", query) }

    suspend fun searchHistory(): List<SearchHistoryEntry> =
        getList(SearchHistoryEntry.serializer(), "me", "search-history")

    suspend fun forgetSearches(tokens: List<String>) =
        post("me", "search-history", "forget", body = buildJsonObject { putJsonArray("tokens") { tokens.forEach { add(JsonPrimitive(it)) } } })

    suspend fun remoteQueue(): RemoteQueue = get(RemoteQueue.serializer(), "me", "remote-queue")

    suspend fun cacheUsage(): CacheUsage = get(CacheUsage.serializer(), "cache")

    suspend fun clearCache(): CacheUsage = call(CacheUsage.serializer(), "DELETE", null, "cache")

    suspend fun account(): Account? = get(Me.serializer(), "me").account

    suspend fun album(id: String): Album = get(Album.serializer(), "albums", id)

    suspend fun artist(id: String): Artist = get(Artist.serializer(), "artists", id)

    suspend fun playlistPage(id: String, continuation: String = ""): PlaylistPage =
        get(PlaylistPage.serializer(), "playlists", id) {
            addQueryParameter("paged", "1")
            if (continuation.isNotEmpty()) addQueryParameter("continuation", continuation)
        }

    /** A whole playlist, however many pages it takes; a token seen before ends it. */
    suspend fun completePlaylist(id: String): Playlist {
        val first = playlistPage(id)
        val tracks = first.playlist.tracks.toMutableList()
        val seen = mutableSetOf(first.next)
        var cursor = first.next
        repeat(PLAYLIST_PAGES_MOST) {
            if (cursor.isEmpty()) return@repeat
            val page = playlistPage(id, cursor)
            tracks += page.playlist.tracks
            if (!seen.add(page.next)) cursor = "" else cursor = page.next
        }
        return first.playlist.copy(tracks = tracks)
    }

    /** The song and its music video, as YouTube pairs them; looks a song up by its id. */
    suspend fun versions(trackId: String): List<Track> = getList(Track.serializer(), "tracks", trackId, "versions")

    suspend fun podcast(id: String): Podcast = get(Podcast.serializer(), "podcasts", id)

    suspend fun radioOf(trackId: String): List<Track> = getList(Track.serializer(), "radio", trackId)

    suspend fun search(query: String, filter: SearchFilter = SearchFilter.All): SearchResults =
        get(SearchResults.serializer(), "search") {
            addQueryParameter("q", query)
            addQueryParameter("filter", filter.wire)
        }

    suspend fun library(): List<LibraryItem> = getList(LibraryItem.serializer(), "me", "library")

    suspend fun folders(): List<Folder> = getList(Folder.serializer(), "me", "folders")

    suspend fun createFolder(name: String) = post("me", "folders", body = buildJsonObject { put("name", name) })

    suspend fun deleteFolder(id: String) = call<Unit>(null, "DELETE", null, "me", "folders", id)

    /** Pins an item or files it in a folder; null leaves that as it is. */
    suspend fun organise(kind: LibraryKind, itemId: String, pinned: Boolean? = null, folderId: String? = null) =
        post("me", "library", "organise", body = buildJsonObject {
            put("kind", kind.wire)
            put("itemId", itemId)
            pinned?.let { put("pinned", it) }
            folderId?.let { put("folderId", it) }
        })

    suspend fun mixes(): List<Mix> = getList(Mix.serializer(), "me", "mixes")

    suspend fun liked(): Playlist = get(Playlist.serializer(), "me", "liked")

    /** The words of a track, timed where a source has them; null when none has any. */
    suspend fun lyrics(track: Track): Lyrics? = try {
        get(Lyrics.serializer(), "tracks", track.id, "lyrics") {
            addQueryParameter("title", track.title)
            addQueryParameter("artist", track.artistNames)
            addQueryParameter("album", track.album?.name.orEmpty())
            addQueryParameter("durationMs", track.durationMs.toString())
            addQueryParameter("timed", "1")
            if (track.isVideo) addQueryParameter("video", "1")
        }.withLines()
    } catch (e: ApiError.Status) {
        if (e.code == 404) null else throw e
    }

    suspend fun setLiked(trackId: String, liked: Boolean) =
        post("me", "tracks", trackId, "rating", body = buildJsonObject { put("rating", if (liked) "like" else "none") })

    suspend fun addToPlaylist(playlistId: String, trackIds: List<String>) =
        post("me", "playlists", playlistId, "tracks", body = buildJsonObject {
            putJsonArray("trackIds") { trackIds.forEach { add(JsonPrimitive(it)) } }
        })

    suspend fun deletePlaylist(playlistId: String) = call<Unit>(null, "DELETE", null, "me", "playlists", playlistId)

    /** Removes entries, each a track id with its place's id: a song can be in a list twice. */
    suspend fun removeFromPlaylist(playlistId: String, items: List<Pair<String, String>>) =
        call<Unit>(null, "DELETE", buildJsonObject {
            putJsonArray("items") {
                items.forEach { (track, item) ->
                    add(buildJsonObject { put("trackId", track); put("itemId", item) })
                }
            }
        }, "me", "playlists", playlistId, "tracks")

    suspend fun setFollowing(artistId: String, follow: Boolean) =
        post("me", "artists", artistId, "follow", body = buildJsonObject { put("follow", follow) })

    /** Creates a private playlist and returns its id. */
    suspend fun createPlaylist(title: String): String {
        return call(Created.serializer(), "POST", buildJsonObject {
            put("title", title); put("description", ""); put("public", false)
        }, "me", "playlists").id
    }

    suspend fun authReload(): Boolean {
        return call(Reloaded.serializer(), "POST", JsonObject(emptyMap()), "auth", "reload").signedIn
    }

    suspend fun signOut() = post("auth", "sign-out", body = JsonObject(emptyMap()))

    suspend fun prefetch(trackId: String) = post("prefetch", trackId, body = JsonObject(emptyMap()))

    /** Why a track last failed to resolve: a rate limit is waited out, not skipped. */
    suspend fun health(trackId: String): Health = get(Health.serializer(), "tracks", trackId, "health")

    /** The track's loudness in LKFS, when YouTube gives one (signed in). */
    suspend fun loudness(trackId: String): Double? =
        runCatching { get(Loudness.serializer(), "tracks", trackId, "loudness").loudnessLkfs }.getOrNull()

    // ---------- your listening ----------

    suspend fun stats(days: Int): Stats {
        fun top(what: String, limit: Int): HttpUrl.Builder.() -> Unit = {
            addQueryParameter("days", days.toString()); addQueryParameter("limit", limit.toString())
        }
        return Stats(
            summary = get(Summary.serializer(), "me", "stats", "summary") { addQueryParameter("days", days.toString()) },
            tracks = getList(TrackStat.serializer(), "me", "stats", "tracks", query = top("tracks", 50)),
            artists = getList(ArtistStat.serializer(), "me", "stats", "artists", query = top("artists", 20)),
            albums = getList(AlbumStat.serializer(), "me", "stats", "albums", query = top("albums", 20)),
            onRepeat = getList(TrackStat.serializer(), "me", "stats", "on-repeat") { addQueryParameter("limit", "20") },
        )
    }

    suspend fun statDetail(kind: String, id: String): StatDetail =
        get(StatDetail.serializer(), "me", "stats", "detail") {
            addQueryParameter("kind", kind); addQueryParameter("id", id)
        }

    // ---------- plumbing ----------

    internal suspend fun <T> get(
        serializer: KSerializer<T>,
        vararg path: String,
        query: HttpUrl.Builder.() -> Unit = {},
    ): T = call(serializer, "GET", null, *path, query = query)

    internal suspend fun <T> getList(
        serializer: KSerializer<T>,
        vararg path: String,
        query: HttpUrl.Builder.() -> Unit = {},
    ): List<T> = call(ListSerializer(serializer).nullable, "GET", null, *path, query = query).orEmpty()

    internal suspend fun post(vararg path: String, body: JsonElement) {
        call<Unit>(null, "POST", body, *path)
    }

    /**
     * One request. Path segments are added encoded, so an id from anywhere
     * (a shared link, another app) can only ever name one segment of a route.
     */
    internal suspend fun <T> call(
        serializer: KSerializer<T>?,
        method: String,
        body: JsonElement?,
        vararg path: String,
        query: HttpUrl.Builder.() -> Unit = {},
    ): T = withContext(Dispatchers.IO) {
        val url = origin().newBuilder().addPathSegment("v1").apply {
            path.forEach { addPathSegment(it) }
            query()
        }.build()
        val requestBody = body?.let { CoreJson.encodeToString(JsonElement.serializer(), it).toRequestBody(JSON) }
        val request = Request.Builder().url(url).method(method, requestBody).build()
        val (code, text) = try {
            http.newCall(request).execute().use { it.code to it.body.string() }
        } catch (e: IOException) {
            throw ApiError.Unreachable(e.message.orEmpty())
        }
        if (code !in 200..299) throw errorFor(code, text)
        @Suppress("UNCHECKED_CAST")
        if (serializer == null) return@withContext Unit as T
        try {
            CoreJson.decodeFromString(serializer, text)
        } catch (e: Exception) {
            throw ApiError.Decode("${url.encodedPath}: ${e.message}")
        }
    }

    companion object {
        private val JSON = "application/json".toMediaType()
        private const val PLAYLIST_PAGES_MOST = 60

        fun errorFor(code: Int, text: String): ApiError {
            val body = runCatching { CoreJson.decodeFromString(ErrorBody.serializer(), text) }.getOrDefault(ErrorBody())
            return when {
                code == 401 || body.reauth -> ApiError.SignedOut
                code == 429 || body.rateLimited -> ApiError.RateLimited
                else -> ApiError.Status(code, body.error)
            }
        }

        fun originOf(address: String): HttpUrl = "http://$address/".toHttpUrl()
    }
}
