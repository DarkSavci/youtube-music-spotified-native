package dev.darksavci.spotified.data

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.doubleOrNull
import okhttp3.HttpUrl.Companion.toHttpUrl
import okhttp3.OkHttpClient
import okhttp3.Request

/**
 * SponsorBlock's public database: the parts of a YouTube upload that are not
 * the music (an intro, talk, a sponsor's read), which listeners have marked.
 * Official releases have none; a song that comes from an upload often does.
 */
object SponsorBlock {
    private const val API = "https://sponsor.ajay.app/api/skipSegments"
    private val CATEGORIES = """["sponsor","selfpromo","intro","outro","interaction","music_offtopic"]"""

    /** The stretches to skip, in milliseconds; empty when none are marked or the database cannot be reached. */
    suspend fun segments(http: OkHttpClient, videoId: String): List<LongRange> = withContext(Dispatchers.IO) {
        val url = API.toHttpUrl().newBuilder()
            .addQueryParameter("videoID", videoId)
            .addQueryParameter("categories", CATEGORIES)
            .build()
        runCatching {
            http.newCall(Request.Builder().url(url).build()).execute().use { r ->
                if (!r.isSuccessful) return@use emptyList()
                (Json.parseToJsonElement(r.body.string()) as? JsonArray).orEmpty().mapNotNull { s ->
                    val pair = ((s as? JsonObject)?.get("segment") as? JsonArray) ?: return@mapNotNull null
                    val start = (pair.getOrNull(0) as? JsonPrimitive)?.doubleOrNull ?: return@mapNotNull null
                    val end = (pair.getOrNull(1) as? JsonPrimitive)?.doubleOrNull ?: return@mapNotNull null
                    (start * 1000).toLong()..(end * 1000).toLong()
                }
            }
        }.getOrDefault(emptyList())
    }
}
