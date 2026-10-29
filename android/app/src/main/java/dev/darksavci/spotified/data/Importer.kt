package dev.darksavci.spotified.data

import dev.darksavci.spotified.api.Api
import dev.darksavci.spotified.api.Item
import dev.darksavci.spotified.api.SearchFilter
import dev.darksavci.spotified.api.Track
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.async
import kotlinx.coroutines.awaitAll
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.sync.Semaphore
import kotlinx.coroutines.sync.withPermit
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import okhttp3.HttpUrl.Companion.toHttpUrlOrNull
import okhttp3.OkHttpClient
import okhttp3.Request

/**
 * A playlist from another service brought here: its songs read from the link
 * (Deezer's API, Spotify's and Apple Music's pages) or from pasted lines, each
 * looked for here by title and artist.
 */
object Importer {
    data class Wanted(val title: String, val artist: String) {
        override fun toString() = if (artist.isEmpty()) title else "$artist - $title"
    }

    data class Result(
        val name: String,
        val wanted: Int,
        /** Everything to put in the playlist, in order. */
        val found: List<Track>,
        /** Songs found whose names do not quite match: shown for a check. */
        val doubtful: List<Pair<String, Track>>,
        val missing: List<String>,
    )

    private const val UA = "Mozilla/5.0 (Linux; Android 14) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0 Mobile Safari/537.36"

    suspend fun match(http: OkHttpClient, api: Api, input: String): Result {
        val (name, songs) = read(http, input.trim())
        if (songs.isEmpty()) error("No songs were found in that.")
        val gate = Semaphore(4)
        val matched = coroutineScope {
            songs.map { w -> async { gate.withPermit { w to runCatching { find(api, w) }.getOrNull() } } }.awaitAll()
        }
        val found = mutableListOf<Track>()
        val doubtful = mutableListOf<Pair<String, Track>>()
        val missing = mutableListOf<String>()
        for ((w, hit) in matched) {
            when {
                hit == null -> missing += w.toString()
                else -> {
                    found += hit.first
                    if (!hit.second) doubtful += w.toString() to hit.first
                }
            }
        }
        return Result(name, songs.size, found, doubtful, missing)
    }

    /** The best song here for [w], and whether its names match closely. */
    private suspend fun find(api: Api, w: Wanted): Pair<Track, Boolean>? {
        val results = api.search("${w.artist} ${w.title}".trim(), SearchFilter.Songs)
        val songs = (listOfNotNull(results.topResult) + results.shelves.flatMap { it.items }).filterIsInstance<Item.Song>().map { it.track }
        val close = songs.firstOrNull { Services.alike(it.title, w.title) && (w.artist.isEmpty() || artistMatches(it, w.artist)) }
        return close?.let { it to true } ?: songs.firstOrNull()?.let { it to false }
    }

    private fun artistMatches(t: Track, artist: String): Boolean {
        val want = Services.plain(artist)
        return t.artists.any { a -> Services.plain(a.name).let { it.contains(want) || want.contains(it) } }
    }

    private suspend fun read(http: OkHttpClient, input: String): Pair<String, List<Wanted>> = withContext(Dispatchers.IO) {
        val link = Regex("https?://\\S+").find(input)?.value ?: return@withContext "Imported" to lines(input)
        // Exactly these hosts, over HTTPS: a link is fetched only where it says it is from.
        val url = link.toHttpUrlOrNull()?.takeIf { it.isHttps } ?: error("That is not a playlist link this app can read.")
        when (url.host.removePrefix("www.")) {
            "deezer.com", "link.deezer.com", "deezer.page.link" -> deezer(http, link)
            "open.spotify.com" -> spotify(http, link)
            "music.apple.com" -> apple(http, link)
            else -> error("That is not a playlist link this app can read.")
        }
    }

    /** "Artist - Title" or "Title, Artist" a line; a CSV export's header is skipped. */
    private fun lines(text: String) = text.lines().map { it.trim() }.filter { it.isNotEmpty() }
        .filterNot { it.lowercase().startsWith("track name") || it.lowercase().startsWith("\"track") }
        .map { line ->
            when {
                " - " in line -> line.split(" - ", limit = 2).let { Wanted(it[1].trim(), it[0].trim()) }
                "," in line -> line.split(",").map { it.trim('"', ' ') }.let { Wanted(it[0], it.getOrElse(1) { "" }) }
                else -> Wanted(line, "")
            }
        }

    private fun deezer(http: OkHttpClient, link: String): Pair<String, List<Wanted>> {
        // A short link is followed, but only to where Deezer itself sends it.
        val id = Regex("playlist/(\\d+)").find(link)?.groupValues?.get(1)
            ?: page(http, link).first.takeIf { it.toHttpUrlOrNull()?.host?.endsWith("deezer.com") == true }
                ?.let { Regex("playlist/(\\d+)").find(it)?.groupValues?.get(1) }
            ?: error("That Deezer link is not to a playlist.")
        val playlist = json(http, "https://api.deezer.com/playlist/$id")
        val name = (playlist["title"] as? JsonPrimitive)?.content ?: "Imported"
        val songs = mutableListOf<Wanted>()
        var next: String? = "https://api.deezer.com/playlist/$id/tracks?limit=100"
        while (next != null && songs.size < 1000) {
            val pageJson = json(http, next)
            (pageJson["data"] as? JsonArray).orEmpty().mapNotNull { it as? JsonObject }.forEach { t ->
                songs += Wanted((t["title"] as? JsonPrimitive)?.content.orEmpty(), ((t["artist"] as? JsonObject)?.get("name") as? JsonPrimitive)?.content.orEmpty())
            }
            next = (pageJson["next"] as? JsonPrimitive)?.content
        }
        return name to songs
    }

    /** Spotify's page names each song as a music:song link; each song's page names it and its artist. */
    private fun spotify(http: OkHttpClient, link: String): Pair<String, List<Wanted>> {
        val (_, html) = page(http, link)
        val name = meta(html, "og:title").firstOrNull() ?: "Imported"
        // Only Spotify's own song pages are followed.
        val links = meta(html, "music:song").filter { it.startsWith("https://open.spotify.com/track/") }.take(100)
        if (links.isEmpty()) error("Spotify did not list that playlist's songs.")
        val songs = links.mapNotNull { song ->
            runCatching {
                val p = page(http, song).second
                Wanted(meta(p, "og:title").first(), meta(p, "music:musician_description").firstOrNull()
                    ?: meta(p, "og:description").firstOrNull()?.split(" · ")?.firstOrNull().orEmpty())
            }.getOrNull()
        }
        return name to songs
    }

    /** Apple Music's page carries the playlist as structured data. */
    private fun apple(http: OkHttpClient, link: String): Pair<String, List<Wanted>> {
        val html = page(http, link).second
        val name = meta(html, "og:title").firstOrNull()?.removeSuffix(" on Apple Music") ?: "Imported"
        val ld = Regex("<script[^>]*application/ld\\+json[^>]*>(.*?)</script>", RegexOption.DOT_MATCHES_ALL).findAll(html)
            .mapNotNull { runCatching { Json.parseToJsonElement(it.groupValues[1]) as? JsonObject }.getOrNull() }
            .firstOrNull { "track" in it } ?: error("Apple Music did not list that playlist's songs.")
        val songs = (ld["track"] as? JsonArray).orEmpty().mapNotNull { it as? JsonObject }.map { t ->
            val artist = ((t["byArtist"] as? JsonArray)?.firstOrNull() as? JsonObject ?: t["byArtist"] as? JsonObject)
                ?.get("name") as? JsonPrimitive
            Wanted((t["name"] as? JsonPrimitive)?.content.orEmpty(), artist?.content.orEmpty())
        }
        return name to songs
    }

    private fun page(http: OkHttpClient, url: String): Pair<String, String> =
        http.newCall(Request.Builder().url(url).header("User-Agent", UA).build()).execute().use { r ->
            if (!r.isSuccessful) error("${r.request.url.host} answered ${r.code}")
            r.request.url.toString() to r.body.string()
        }

    private fun json(http: OkHttpClient, url: String): JsonObject =
        http.newCall(Request.Builder().url(url).build()).execute().use { Json.parseToJsonElement(it.body.string()) as JsonObject }

    private fun meta(html: String, key: String): List<String> =
        Regex("<meta[^>]+(?:property|name)=\"${Regex.escape(key)}\"[^>]+content=\"([^\"]*)\"").findAll(html)
            .map { it.groupValues[1].replace("&amp;", "&").replace("&quot;", "\"").replace("&#x27;", "'").replace("&#39;", "'") }
            .toList()
}
