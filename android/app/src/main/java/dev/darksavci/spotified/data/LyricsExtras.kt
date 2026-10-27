package dev.darksavci.spotified.data

import android.icu.text.Transliterator
import android.os.Build
import java.util.Locale
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonPrimitive
import okhttp3.HttpUrl.Companion.toHttpUrl
import okhttp3.OkHttpClient
import okhttp3.Request

/**
 * A second line under each line of lyrics: the words romanised, done on the
 * phone with ICU, or translated into the phone's language.
 *
 * Translation asks Google's public translate endpoint, the one its own
 * widgets use. It is off unless the listener turns it on, since it sends the
 * lyrics there.
 */
object LyricsExtras {
    suspend fun of(http: OkHttpClient, lines: List<String>, romanise: Boolean, translate: Boolean): List<String>? = withContext(Dispatchers.Default) {
        when {
            translate -> runCatching { translated(http, lines) }.getOrNull()
            romanise -> romanised(lines)
            else -> null
        }
    }

    /** Lines in another script, in Latin letters; null where ICU is not available (before Android 10). */
    fun romanised(lines: List<String>): List<String>? {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.Q) return null
        val latin = Transliterator.getInstance("Any-Latin; Latin-ASCII")
        return lines.map { if (it.all { c -> c.code < 0x250 }) "" else latin.transliterate(it) }
    }

    private suspend fun translated(http: OkHttpClient, lines: List<String>): List<String> = withContext(Dispatchers.IO) {
        val language = Locale.getDefault().language.ifEmpty { "en" }
        // In batches, each line on its own line, so the answer splits back.
        lines.chunked(40).flatMap { batch ->
            val url = "https://translate.googleapis.com/translate_a/single".toHttpUrl().newBuilder()
                .addQueryParameter("client", "gtx")
                .addQueryParameter("sl", "auto")
                .addQueryParameter("tl", language)
                .addQueryParameter("dt", "t")
                .addQueryParameter("q", batch.joinToString("\n"))
                .build()
            val body = http.newCall(Request.Builder().url(url).build()).execute().use { it.body.string() }
            val parts = Json.parseToJsonElement(body).jsonArray[0] as JsonArray
            val text = parts.joinToString("") { it.jsonArray[0].jsonPrimitive.content }
            val out = text.split("\n")
            batch.indices.map { out.getOrElse(it) { "" } }
        }
    }
}
