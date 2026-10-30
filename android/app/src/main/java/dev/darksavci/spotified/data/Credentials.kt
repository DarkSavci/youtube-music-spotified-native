package dev.darksavci.spotified.data

import java.io.File
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.put

/**
 * The account's sign-in, in the credentials.json the core reads: what a sign-in
 * in the WebView writes, what is brought from the desktop, and which channel it
 * acts as. Nothing here logs, and no message says anything of what a file held:
 * the cookies are the account.
 */
object Credentials {
    const val FILE = "credentials.json"

    /** The most a credentials file can sensibly hold. Anything larger was the wrong file. */
    const val MOST_BYTES = 256 * 1024

    /** The cookies the desktop keeps (crates/app/src/signin.rs), and only those. */
    private val KEEP = setOf(
        "SAPISID", "__Secure-1PAPISID", "__Secure-3PAPISID", "SID", "__Secure-1PSID",
        "__Secure-3PSID", "HSID", "SSID", "APISID", "LOGIN_INFO", "__Secure-1PSIDTS",
        "__Secure-3PSIDTS", "SIDCC", "__Secure-1PSIDCC", "__Secure-3PSIDCC",
        "VISITOR_INFO1_LIVE", "VISITOR_PRIVACY_METADATA", "PREF", "YSC", "__Secure-YNID",
        "__Secure-ROLLOUT_TOKEN", "__Secure-BUCKET", "wide",
    )

    /** The field that names the channel in use. Absent, or empty, for the account's own. */
    private const val CHANNEL = "onBehalfOfUser"

    private val json = Json { ignoreUnknownKeys = true }

    /** What came of reading a file: a sign-in to write, or why not, in words safe to show. */
    sealed interface Read {
        data class Good(val json: String) : Read
        data class Bad(val message: String) : Read
    }

    /** An account, and a cookie request signatures are computed over. */
    private fun signedIn(names: Set<String>) = "LOGIN_INFO" in names && names.any { it.endsWith("SAPISID") }

    private fun namesIn(header: String) = header.split(";").map { it.trim().substringBefore('=') }.filter { it.isNotEmpty() }.toSet()

    /** The sign-in the WebView's cookie header makes; null while it is not signed in. */
    fun fromCookies(header: String): String? {
        val pairs = header.split("; ").map { it.substringBefore('=') to it }.filter { (name, _) -> name in KEEP }.distinctBy { it.first }
        if (!signedIn(pairs.map { it.first }.toSet())) return null
        return buildJsonObject { put("cookie", pairs.joinToString("; ") { it.second }) }.toString()
    }

    /**
     * A file from the desktop: {"cookie": ..., "extra": {...}} as it writes one, or
     * the flat map of headers browser tools produce, as the core itself accepts
     * (core/internal/innertube/auth.go). What the core would ignore is left out.
     */
    fun fromFile(text: String): Read {
        val document = runCatching { json.parseToJsonElement(text.removePrefix("﻿")) as? JsonObject }.getOrNull()
            ?: return Read.Bad("That is not a credentials file: it should be credentials.json from the desktop app.")
        val fields = document.entries.associate { it.key.lowercase() to it.value }
        val cookie = fields["cookie"]?.let(::textOf).orEmpty()
        if (cookie.isEmpty()) return Read.Bad("That file holds no sign-in.")
        if ('\r' in cookie || '\n' in cookie) return Read.Bad("That file's sign-in is damaged.")
        if (!signedIn(namesIn(cookie))) return Read.Bad("That file's sign-in is not complete. Sign in on the desktop app again and copy it afresh.")
        val extra = (fields["extra"] as? JsonObject)?.entries.orEmpty().associate { it.key.lowercase() to textOf(it.value) } +
            fields.filterKeys { it !in setOf("cookie", "extra", CHANNEL.lowercase()) }.mapValues { textOf(it.value) }
        val channel = fields[CHANNEL.lowercase()]?.let(::textOf).orEmpty()
        return Read.Good(
            buildJsonObject {
                put("cookie", cookie)
                if (channel.isNotEmpty()) put(CHANNEL, channel)
                val kept = extra.filterValues { it.isNotEmpty() }
                if (kept.isNotEmpty()) put("extra", buildJsonObject { kept.forEach { (k, v) -> put(k, v) } })
            }.toString(),
        )
    }

    private fun textOf(value: JsonElement) = (value as? JsonPrimitive)?.contentOrNull.orEmpty()

    /** The channel a sign-in names; empty for the account's own, and for no sign-in. */
    fun activeChannel(text: String): String =
        runCatching { (json.parseToJsonElement(text) as JsonObject)[CHANNEL]?.jsonPrimitive?.contentOrNull.orEmpty() }.getOrDefault("")

    /** The sign-in naming [channel], with the rest as it was. Empty is the account's own. */
    fun withChannel(text: String, channel: String): String {
        val fields = (json.parseToJsonElement(text) as JsonObject).toMutableMap()
        if (channel.isEmpty()) fields.remove(CHANNEL) else fields[CHANNEL] = JsonPrimitive(channel)
        return JsonObject(fields).toString()
    }

    /** Written beside the file and renamed over it, so a crash mid-write cannot leave half a sign-in. */
    fun write(dir: File, text: String) {
        val partial = File(dir, "$FILE.part")
        partial.writeText(text)
        check(partial.renameTo(File(dir, FILE))) { "The sign-in could not be saved." }
    }

    fun read(dir: File): String? = File(dir, FILE).takeIf { it.isFile }?.readText()
}
