package dev.darksavci.spotified.data

import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.contentOrNull

/**
 * The songs in a file exported from another service: CSV or TSV with a header
 * (Exportify, TuneMyMusic, Soundiiz, Apple Music's "Export Playlist"), Spotify's
 * own data export as JSON, or an M3U playlist. Anything else is read as the lines
 * pasted into the page are.
 */
object ExportFile {
    /** The most a playlist export can sensibly hold. */
    const val MOST_BYTES = 8 * 1024 * 1024

    private val TITLE = listOf("track name", "track title", "title", "name", "song", "song name", "track")
    private val ARTIST = listOf("artist name(s)", "artist name", "artist names", "artists", "artist", "album artist")

    /** Text from a file's bytes: Apple Music writes UTF-16 with a mark, the rest UTF-8. */
    fun decode(bytes: ByteArray): String = when {
        bytes.size >= 2 && bytes[0] == 0xFF.toByte() && bytes[1] == 0xFE.toByte() -> String(bytes, 2, bytes.size - 2, Charsets.UTF_16LE)
        bytes.size >= 2 && bytes[0] == 0xFE.toByte() && bytes[1] == 0xFF.toByte() -> String(bytes, 2, bytes.size - 2, Charsets.UTF_16BE)
        else -> String(bytes, Charsets.UTF_8).removePrefix("﻿")
    }

    /** The playlist's name, if the file says, and its songs in order. Empty when nothing in it reads as a song. */
    fun read(fileName: String, text: String): Pair<String, List<Importer.Wanted>> {
        val base = fileName.substringBeforeLast('.').ifBlank { "Imported" }
        val trimmed = text.trimStart()
        val songs = when {
            trimmed.startsWith("{") || trimmed.startsWith("[") -> spotifyJson(trimmed)?.let { (name, list) -> return (name ?: base) to list }
            trimmed.startsWith("#EXTM3U") -> m3u(trimmed)
            else -> table(text)
        }
        return base to (songs ?: Importer.lines(text))
    }

    /** A table with a header naming its columns; null when the first line names none. */
    private fun table(text: String): List<Importer.Wanted>? {
        val first = text.lineSequence().firstOrNull { it.isNotBlank() } ?: return null
        val separator = listOf('\t', ';', ',').maxByOrNull { c -> first.count { it == c } } ?: ','
        val rows = rows(text, separator)
        val header = rows.firstOrNull()?.map { it.trim().lowercase() } ?: return null
        val title = TITLE.firstNotNullOfOrNull { name -> header.indexOf(name).takeIf { it >= 0 } } ?: return null
        val artist = ARTIST.firstNotNullOfOrNull { name -> header.indexOf(name).takeIf { it >= 0 } }
        return rows.drop(1).mapNotNull { row ->
            val name = row.getOrNull(title)?.trim().orEmpty()
            // Several artists are one field, split by the exporter's own mark; the first is enough to find the song.
            val by = artist?.let { row.getOrNull(it) }?.split(';', '|')?.firstOrNull()?.trim().orEmpty()
            if (name.isEmpty()) null else Importer.Wanted(name, by)
        }
    }

    /** Splits a table by [separator], with quoted fields that may hold it, a doubled quote or a line break. */
    fun rows(text: String, separator: Char = ','): List<List<String>> {
        val rows = mutableListOf<List<String>>()
        var row = mutableListOf<String>()
        val field = StringBuilder()
        var quoted = false
        var i = 0
        fun endField() { row.add(field.toString()); field.clear() }
        fun endRow() {
            endField()
            if (row.any { it.isNotBlank() }) rows.add(row)
            row = mutableListOf()
        }
        while (i < text.length) {
            val c = text[i]
            when {
                quoted && c == '"' && text.getOrNull(i + 1) == '"' -> { field.append('"'); i++ }
                c == '"' && (quoted || field.isEmpty()) -> quoted = !quoted
                quoted -> field.append(c)
                c == separator -> endField()
                c == '\n' -> endRow()
                c == '\r' -> Unit
                else -> field.append(c)
            }
            i++
        }
        if (field.isNotEmpty() || row.isNotEmpty()) endRow()
        return rows
    }

    /** "#EXTINF:length,Artist - Title" before each entry. */
    private fun m3u(text: String): List<Importer.Wanted> = text.lineSequence()
        .filter { it.startsWith("#EXTINF") }
        .map { it.substringAfter(',', "").trim() }
        .filter { it.isNotEmpty() }
        .map { line ->
            if (" - " in line) line.split(" - ", limit = 2).let { Importer.Wanted(it[1].trim(), it[0].trim()) } else Importer.Wanted(line, "")
        }.toList()

    /** Spotify's data export: its Playlist1.json, or YourLibrary.json. Null when it is neither. */
    private fun spotifyJson(text: String): Pair<String?, List<Importer.Wanted>>? {
        val root = runCatching { Json.parseToJsonElement(text) as? JsonObject }.getOrNull() ?: return null
        (root["playlists"] as? JsonArray)?.firstOrNull()?.let { it as? JsonObject }?.let { playlist ->
            val songs = (playlist["items"] as? JsonArray).orEmpty().mapNotNull { item ->
                val track = (item as? JsonObject)?.get("track") as? JsonObject ?: return@mapNotNull null
                val name = textOf(track["trackName"])
                if (name.isEmpty()) null else Importer.Wanted(name, textOf(track["artistName"]))
            }
            return textOf(playlist["name"]).ifEmpty { null } to songs
        }
        (root["tracks"] as? JsonArray)?.let { tracks ->
            return null to tracks.mapNotNull { item ->
                val track = item as? JsonObject ?: return@mapNotNull null
                val name = textOf(track["track"])
                if (name.isEmpty()) null else Importer.Wanted(name, textOf(track["artist"]))
            }
        }
        return null
    }

    private fun textOf(value: JsonElement?) = (value as? JsonPrimitive)?.contentOrNull.orEmpty()
}
