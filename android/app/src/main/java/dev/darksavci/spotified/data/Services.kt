package dev.darksavci.spotified.data

import dev.darksavci.spotified.BuildConfig
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.longOrNull
import okhttp3.HttpUrl.Companion.toHttpUrl
import okhttp3.OkHttpClient
import okhttp3.Request
import java.util.Locale

/**
 * Other music services: a link from one read as what to search for here,
 * and a song, album or artist found there to share. A port of
 * crates/app/src/services.rs; the services' own public lookups are asked,
 * never an account.
 */
object Services {
    enum class Service(val label: String) { Spotify("Spotify"), AppleMusic("Apple Music"), Deezer("Deezer") }

    /** What is to be found elsewhere. [album] may be empty; [songs] are some of an album's titles. */
    sealed interface Wanted {
        data class Song(val title: String, val artist: String, val album: String) : Wanted
        data class Album(val title: String, val artist: String, val songs: List<String>) : Wanted
        data class Artist(val name: String, val channel: String) : Wanted
    }

    private const val USER_AGENT = "spotified-android/${BuildConfig.VERSION_NAME} ( https://github.com/DarkSavci/youtube-music-spotified-native )"

    fun isLink(text: String) = source(text.trim()) != null

    /** Where a pasted link is from. SoundCloud is read, but not offered to share to. */
    private enum class Source { Spotify, AppleMusic, Deezer, SoundCloud }

    /** What to search for here to find what [link] is to. */
    suspend fun read(http: OkHttpClient, link: String): String = withContext(Dispatchers.IO) {
        val text = link.trim()
        val (title, artist) = when (source(text) ?: error("That is not a link this app can read")) {
            Source.Spotify -> spotifyRead(http, spotifyWebLink(text))
            Source.AppleMusic -> appleRead(http, text)
            Source.Deezer -> deezerRead(http, text)
            Source.SoundCloud -> soundCloudRead(http, text)
        }
        "$title $artist".trim()
    }

    private fun source(text: String): Source? {
        if (text.any { it.isWhitespace() }) return null
        if (text.startsWith("spotify:")) return Source.Spotify
        return when (host(text)?.removePrefix("www.")) {
            "open.spotify.com", "spotify.link" -> Source.Spotify
            "music.apple.com", "itunes.apple.com" -> Source.AppleMusic
            "deezer.com", "link.deezer.com", "deezer.page.link" -> Source.Deezer
            "soundcloud.com", "m.soundcloud.com", "on.soundcloud.com" -> Source.SoundCloud
            else -> null
        }
    }

    /** SoundCloud says what a link is to in its preview: "Flickermood by Forss". */
    private fun soundCloudRead(http: OkHttpClient, link: String): Pair<String, String> {
        val answer = getJson(http, "https://soundcloud.com/oembed", "format" to "json", "url" to link)
        val author = answer.text("author_name")
        val title = answer.text("title").ifEmpty { error("SoundCloud did not name that link") }
        return title.removeSuffix(" by $author") to author
    }

    /** The address of [wanted] on [service], or null when it is not there. */
    suspend fun find(http: OkHttpClient, service: Service, wanted: Wanted): String? = withContext(Dispatchers.IO) {
        when (service) {
            Service.Spotify -> spotifyFind(http, wanted)
            Service.AppleMusic -> appleFind(http, wanted)
            Service.Deezer -> deezerFind(http, wanted)
        }
    }

    // ---------- Spotify ----------

    private const val LISTENBRAINZ = "https://labs.api.listenbrainz.org/spotify-id-from-metadata/json"
    private const val WIKIDATA = "https://query.wikidata.org/sparql"
    private const val SPOTIFY = "https://open.spotify.com"

    private fun spotifyWebLink(text: String) =
        if (text.startsWith("spotify:")) "$SPOTIFY/" + text.removePrefix("spotify:").replace(':', '/') else text

    /** What a Spotify page says of itself to anything that previews it. */
    private fun spotifyRead(http: OkHttpClient, link: String): Pair<String, String> {
        val (landed, page) = getPage(http, link)
        val title = meta(page, "og:title") ?: error("Spotify did not say what that link is to")
        // "Daft Punk · One More Time · Song · 2000"
        val byline = meta(page, "og:description")?.split(" · ")?.firstOrNull().orEmpty()
        return when (meta(page, "og:type")) {
            "music.song" -> title to (meta(page, "music:musician_description") ?: byline)
            "music.album" -> spotifyAlbumName(title) to byline
            "music.playlist" -> title to ""
            else -> if ("artist" in pathParts(landed)) title to "" else error("That Spotify link is not to a song, album, artist or playlist")
        }
    }

    private fun spotifyFind(http: OkHttpClient, wanted: Wanted): String? = when (wanted) {
        is Wanted.Song -> trackIds(http, wanted.artist, wanted.album, wanted.title).firstOrNull()?.let { "$SPOTIFY/track/$it" }
        is Wanted.Album -> spotifyAlbum(http, wanted)
        is Wanted.Artist -> spotifyArtist(http, wanted)
    }

    private fun trackIds(http: OkHttpClient, artist: String, album: String, title: String): List<String> {
        val answer = getJson(http, LISTENBRAINZ, "artist_name" to artist, "release_name" to album, "track_name" to title)
        val first = (answer as? JsonArray)?.firstOrNull() as? JsonObject ?: return emptyList()
        return (first["spotify_track_ids"] as? JsonArray).orEmpty().mapNotNull { (it as? JsonPrimitive)?.content }
    }

    /** An album is found through its songs: each is on some album, which its page names. */
    private fun spotifyAlbum(http: OkHttpClient, wanted: Wanted.Album): String? {
        val seen = mutableSetOf<String>()
        for (song in wanted.songs.take(3)) {
            for (id in trackIds(http, wanted.artist, wanted.title, song).take(4)) {
                val link = meta(getPage(http, "$SPOTIFY/track/$id").second, "music:album") ?: continue
                if (!seen.add(link)) continue
                val name = meta(getPage(http, link).second, "og:title")?.let(::spotifyAlbumName)
                if (name != null && alike(name, wanted.title)) return link
            }
        }
        return null
    }

    /** "Listen to Discovery by Daft Punk | Spotify": the name is what is between. */
    private fun spotifyAlbumName(title: String): String {
        val t = title.removePrefix("Listen to ").removeSuffix(" | Spotify")
        val at = t.lastIndexOf(" by ")
        return albumTitle(if (at >= 0) t.substring(0, at) else t)
    }

    /** Wikidata keeps Spotify's id for many artists: by the YouTube channel when it keeps that too, else by the name. */
    private fun spotifyArtist(http: OkHttpClient, wanted: Wanted.Artist): String? {
        val queries = buildList {
            channelOf(wanted.channel)?.let { add("SELECT DISTINCT ?id WHERE { ?artist wdt:P2397 \"${sparql(it)}\"; wdt:P1902 ?id. }") }
            val name = sparql(wanted.name)
            add("SELECT DISTINCT ?id WHERE { VALUES ?label { \"$name\"@en \"$name\"@mul } ?artist rdfs:label ?label; wdt:P1902 ?id. }")
        }
        for (query in queries) {
            val answer = getJson(http, WIKIDATA, "query" to query, "format" to "json")
            val rows = ((answer.obj()?.obj("results"))?.get("bindings") as? JsonArray).orEmpty()
            val ids = rows.mapNotNull { ((it as? JsonObject)?.obj("id"))?.text("value")?.ifEmpty { null } }.distinct()
            // Two artists of one name: neither is surely the one.
            if (ids.size == 1) return "$SPOTIFY/artist/${ids[0]}"
        }
        return null
    }

    private fun sparql(text: String) = text.replace("\\", "\\\\").replace("\"", "\\\"")

    // ---------- Apple Music ----------

    private const val ITUNES_SEARCH = "https://itunes.apple.com/search"
    private const val ITUNES_LOOKUP = "https://itunes.apple.com/lookup"
    private const val FALLBACK_STORE = "US"

    /** `music.apple.com/tr/album/discovery/697194953?i=697195462` is a song on an album; without `i`, the album. */
    private fun appleRead(http: OkHttpClient, link: String): Pair<String, String> {
        val parts = pathParts(link)
        val store = parts.firstOrNull()?.takeIf { it.length == 2 } ?: FALLBACK_STORE
        if ("playlist" in parts) {
            val title = meta(getPage(http, link).second, "og:title") ?: error("Apple Music did not name that playlist")
            return title.removeSuffix(" on Apple Music") to ""
        }
        val id = (queryValue(link, "i") ?: parts.lastOrNull())?.takeIf { it.all(Char::isDigit) } ?: error("That Apple Music link has no id")
        val result = ((getJson(http, ITUNES_LOOKUP, "id" to id, "country" to store).obj()?.get("results") as? JsonArray)?.firstOrNull() as? JsonObject)
            ?: error("Apple Music did not say what that link is to")
        return when (result.text("wrapperType")) {
            "track" -> result.text("trackName") to result.text("artistName")
            "collection" -> albumTitle(result.text("collectionName")) to result.text("artistName")
            "artist" -> result.text("artistName") to ""
            else -> error("Apple Music did not say what that link is to")
        }
    }

    /** A store is a country, and a song may be sold in one and not another: the phone's own first. */
    private fun appleFind(http: OkHttpClient, wanted: Wanted): String? {
        val own = Locale.getDefault().country.takeIf { it.length == 2 }
        for (store in listOfNotNull(own, FALLBACK_STORE).distinct()) {
            val (term, entity) = when (wanted) {
                is Wanted.Song -> "${wanted.artist} ${wanted.title}" to "song"
                is Wanted.Album -> "${wanted.artist} ${wanted.title}" to "album"
                is Wanted.Artist -> wanted.name to "musicArtist"
            }
            val results = (getJson(http, ITUNES_SEARCH, "term" to term, "entity" to entity, "country" to store, "limit" to "10").obj()?.get("results") as? JsonArray).orEmpty()
            val link = results.mapNotNull { it as? JsonObject }.firstNotNullOfOrNull { r ->
                val (matches, link) = when (wanted) {
                    is Wanted.Song -> (alike(r.text("trackName"), wanted.title) && sameArtist(r.text("artistName"), wanted.artist)) to r.text("trackViewUrl")
                    is Wanted.Album -> (alike(albumTitle(r.text("collectionName")), wanted.title) && sameArtist(r.text("artistName"), wanted.artist)) to r.text("collectionViewUrl")
                    is Wanted.Artist -> (plain(r.text("artistName")) == plain(wanted.name)) to r.text("artistLinkUrl")
                }
                link.takeIf { matches && it.isNotEmpty() }
            }
            if (link != null) return link.replace("?uo=4", "").replace("&uo=4", "")
        }
        return null
    }

    // ---------- Deezer ----------

    private const val DEEZER = "https://api.deezer.com"

    /** `deezer.com/en/track/3135556`; a short link is followed to one. */
    private fun deezerRead(http: OkHttpClient, link: String): Pair<String, String> {
        val landed = if (host(link)?.endsWith("deezer.com") == true) link else getPage(http, link).first
        val parts = pathParts(landed)
        val at = parts.indexOfFirst { it in setOf("track", "album", "artist", "playlist") }
        if (at < 0 || at + 1 >= parts.size) error("That Deezer link is not to a song, album, artist or playlist")
        val kind = parts[at]
        val answer = getJson(http, "$DEEZER/$kind/${parts[at + 1]}").obj() ?: error("Deezer did not answer")
        answer.obj("error")?.text("message")?.takeIf { it.isNotEmpty() }?.let { error("Deezer: $it") }
        return when (kind) {
            "artist" -> answer.text("name") to ""
            "playlist" -> answer.text("title") to ""
            else -> answer.text("title") to (answer.obj("artist")?.text("name").orEmpty())
        }
    }

    private fun deezerFind(http: OkHttpClient, wanted: Wanted): String? {
        val (path, query) = when (wanted) {
            is Wanted.Song -> "search" to "${wanted.artist} ${wanted.title}"
            is Wanted.Album -> "search/album" to "${wanted.artist} ${wanted.title}"
            is Wanted.Artist -> "search/artist" to wanted.name
        }
        val results = (getJson(http, "$DEEZER/$path", "q" to query).obj()?.get("data") as? JsonArray).orEmpty().mapNotNull { it as? JsonObject }
        val matching = results.filter { r ->
            when (wanted) {
                is Wanted.Song -> alike(r.text("title"), wanted.title) && sameArtist(r.obj("artist")?.text("name").orEmpty(), wanted.artist)
                is Wanted.Album -> alike(r.text("title"), wanted.title) && sameArtist(r.obj("artist")?.text("name").orEmpty(), wanted.artist)
                is Wanted.Artist -> plain(r.text("name")) == plain(wanted.name)
            }
        }
        // Many artists share a name, and the one meant is the one listened to.
        val best = if (wanted is Wanted.Artist) matching.maxByOrNull { (it["nb_fan"] as? JsonPrimitive)?.longOrNull ?: 0 } else matching.firstOrNull()
        return best?.text("link")?.ifEmpty { null }
    }

    // ---------- plumbing ----------

    private fun host(text: String): String? {
        val rest = text.removePrefix("https://").takeIf { it != text } ?: text.removePrefix("http://").takeIf { it != text } ?: return null
        return rest.split('/', '?', '#').firstOrNull()?.lowercase()?.ifEmpty { null }
    }

    private fun pathParts(link: String): List<String> =
        link.substringAfter("://").split('?', '#').first().split('/').drop(1).filter { it.isNotEmpty() }

    private fun queryValue(link: String, key: String): String? =
        link.substringAfter('?', "").substringBefore('#').split('&').map { it.split('=', limit = 2) }
            .firstOrNull { it.size == 2 && it[0] == key }?.get(1)

    private fun getJson(http: OkHttpClient, url: String, vararg query: Pair<String, String>): JsonElement {
        val address = url.toHttpUrl().newBuilder().apply { query.forEach { (k, v) -> addQueryParameter(k, v) } }.build()
        val request = Request.Builder().url(address).header("User-Agent", USER_AGENT)
            .header("Accept", "application/json, application/sparql-results+json").build()
        http.newCall(request).execute().use { response ->
            if (!response.isSuccessful) error("${address.host} answered ${response.code}")
            return Json.parseToJsonElement(response.body.string())
        }
    }

    /** A web page: where it ended up after any redirects, and its text. */
    private fun getPage(http: OkHttpClient, url: String): Pair<String, String> {
        val request = Request.Builder().url(url).header("User-Agent", USER_AGENT).header("Accept", "text/html").build()
        http.newCall(request).execute().use { response ->
            if (!response.isSuccessful) error("${host(url).orEmpty()} answered ${response.code}")
            return response.request.url.toString() to response.body.string()
        }
    }

    /** The content of the page's <meta> whose property or name is [key]. */
    private fun meta(page: String, key: String): String? =
        page.split("<meta ").drop(1).firstNotNullOfOrNull { rest ->
            val tag = rest.substringBefore('>')
            val named = attribute(tag, "property") ?: attribute(tag, "name")
            if (named == key) attribute(tag, "content") else null
        }

    private fun attribute(tag: String, name: String): String? {
        val start = tag.indexOf("$name=\"").takeIf { it >= 0 }?.plus(name.length + 2) ?: return null
        val end = tag.indexOf('"', start).takeIf { it >= 0 } ?: return null
        return tag.substring(start, end).replace("&quot;", "\"").replace("&#x27;", "'").replace("&#39;", "'")
            .replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&")
    }

    /** A name as it is compared: lower case, letters and digits, one space between words. */
    internal fun plain(text: String): String =
        text.lowercase().map { c -> if (c == 'ı') 'i' else if (c.isLetterOrDigit()) c else ' ' }.joinToString("")
            .split(' ').filter { it.isNotEmpty() }.joinToString(" ")

    /** Whether a title found is the one wanted, perhaps with more after it ("(Radio Edit)"). */
    internal fun alike(found: String, wanted: String): Boolean {
        val (f, w) = plain(found) to plain(wanted)
        return w.isNotEmpty() && (f == w || f.startsWith("$w ") || w.startsWith("$f "))
    }

    private fun sameArtist(found: String, wanted: String): Boolean {
        val (f, w) = plain(found) to plain(wanted)
        return f.isNotEmpty() && w.isNotEmpty() && (f.contains(w) || w.contains(f))
    }

    private fun albumTitle(title: String) =
        listOf(" - Single", " - EP", " - Album").firstOrNull { title.endsWith(it) }?.let { title.removeSuffix(it) } ?: title

    private fun channelOf(id: String): String? = id.removePrefix("MPLA").takeIf { it.startsWith("UC") }

    private fun JsonElement.obj(): JsonObject? = this as? JsonObject
    private fun JsonObject.obj(key: String): JsonObject? = this[key] as? JsonObject
    private fun JsonElement.text(key: String): String =
        ((this as? JsonObject)?.get(key)?.takeIf { it !is JsonNull } as? JsonPrimitive)?.content.orEmpty()
}
