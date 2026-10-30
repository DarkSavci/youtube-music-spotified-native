package dev.darksavci.spotified.data

import android.content.Context
import android.content.Intent
import android.os.Build
import androidx.core.content.FileProvider
import dev.darksavci.spotified.BuildConfig
import dev.darksavci.spotified.graph
import java.io.File
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import java.util.zip.ZipEntry
import java.util.zip.ZipOutputStream
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/**
 * The problem report: the zip someone sends when something breaks, as the
 * desktop's (crates/app/src/report.rs). Only the logs and a short summary go in,
 * never the database, the credentials or the cookie file; the summary says
 * whether those exist, which is all a diagnosis needs of them. Everything is
 * scrubbed on its way in. It goes where the listener sends it, and nowhere else.
 */
object Report {
    /** What the summary says, gathered from the app. */
    data class Facts(
        val version: String,
        val android: String,
        val phone: String,
        val core: String,
        val signedIn: Boolean,
        val accounts: Int,
        val cache: String,
        val downloads: String,
        val state: String,
        val track: String,
        val queueLength: Int,
        val settings: Settings,
    )

    /** What someone reading the log would ask first. Nothing in it says whose account it is. */
    fun summary(f: Facts): String = Redact.text(
        buildString {
            appendLine("Youtube Music Spotified diagnostics")
            appendLine()
            appendLine("app:         ${f.version}")
            appendLine("android:     ${f.android}")
            appendLine("phone:       ${f.phone}")
            appendLine("core:        ${f.core}")
            appendLine("credentials: ${if (f.signedIn) "present" else "absent"}")
            appendLine("accounts:    ${f.accounts}")
            appendLine("song cache:  ${f.cache}")
            appendLine("downloads:   ${f.downloads}")
            appendLine("player:      ${f.state}; ${f.track.ifEmpty { "nothing" }}; ${f.queueLength} in the queue")
            appendLine()
            appendLine("settings:")
            append(settingsOf(f.settings))
        },
    )

    /**
     * The settings that bear on a fault, named one by one so that a new one is not
     * sent by accident. Who is listened with, and where, are the person's own business.
     */
    fun settingsOf(s: Settings): String = listOf(
        "theme" to s.theme, "crossfadeMs" to s.crossfadeMs, "gapless" to s.gapless, "normalise" to s.normalise,
        "volumeLevel" to s.volumeLevel, "volumeBoost" to s.volumeBoost, "bassBoost" to s.bassBoost,
        "skipSilence" to s.skipSilence, "speed" to s.speed, "autoplay" to s.autoplay,
        "resumeOnLaunch" to s.resumeOnLaunch, "continueFromYouTubeMusic" to s.continueFromYouTubeMusic,
        "reportToYouTube" to s.reportToYouTube, "cacheMaxMb" to s.cacheMaxMb, "downloadOnWifiOnly" to s.downloadOnWifiOnly,
        "equalizerOn" to s.equalizerOn, "lyricsTranslate" to s.lyricsTranslate, "lyricsRomanise" to s.lyricsRomanise,
        "nowPlayingStyle" to s.nowPlayingStyle, "sponsorBlock" to s.sponsorBlock, "wifiQuality" to s.wifiQuality,
        "mobileQuality" to s.mobileQuality, "reduceMotion" to s.reduceMotion,
        "blocked" to "${s.blocked.songs.size} songs, ${s.blocked.artists.size} artists, ${s.blocked.albums.size} albums",
    ).joinToString("") { (name, value) -> "  $name: $value\n" }

    /** Every line of a log, scrubbed, whatever version wrote it. */
    fun scrubbed(log: String): String = log.lineSequence().joinToString("\n") { Redact.text(it) }

    private suspend fun facts(context: Context): Facts {
        val graph = context.graph
        val accounts = graph.core.accounts
        val projection = graph.playback.projection.value
        val queue = projection.state.queue
        val usage = runCatching { graph.api.cacheUsage() }.getOrNull()
        return Facts(
            version = "${BuildConfig.VERSION_NAME} (${BuildConfig.VERSION_CODE})",
            android = "${Build.VERSION.RELEASE} (API ${Build.VERSION.SDK_INT})",
            phone = "${Build.MANUFACTURER} ${Build.MODEL}",
            core = if (usage != null) "answering" else "not answering",
            signedIn = accounts.signedIn(accounts.current),
            accounts = accounts.list().size,
            cache = usage?.let { "${it.tracks} songs, ${sizeText(it.bytes)}" } ?: "unknown",
            downloads = "${graph.downloads.done.value.size} of ${graph.downloads.tracks.value.size} songs, ${sizeText(graph.downloads.bytes.value)}",
            state = "${projection.state.state}",
            track = projection.state.current?.let { "${it.id} \"${it.title}\"" }.orEmpty(),
            queueLength = queue.items.size,
            settings = graph.settings.current(),
        )
    }

    /** An intent that sends the zip, for the system's share sheet. */
    suspend fun shareIntent(context: Context): Intent = withContext(Dispatchers.IO) {
        val summary = summary(facts(context))
        val log = scrubbed(Logs.text())
        val crash = Crashes.pending(context)?.let(::scrubbed)
        val stamp = SimpleDateFormat("yyyyMMdd-HHmmss", Locale.US).format(Date())
        val file = File(shared(context), "spotified-report-$stamp.zip")
        ZipOutputStream(file.outputStream().buffered()).use { zip ->
            fun add(name: String, text: String) {
                zip.putNextEntry(ZipEntry(name))
                zip.write(text.toByteArray())
                zip.closeEntry()
            }
            add("info.txt", summary)
            add("app.log", log)
            crash?.let { add("last-crash.txt", it) }
        }
        send(context, file, "application/zip", "Youtube Music Spotified problem report", "Send the report")
    }

    /** An intent that sends the log alone, scrubbed as a report's is. */
    suspend fun logIntent(context: Context): Intent = withContext(Dispatchers.IO) {
        val file = File(shared(context), "spotified-log.txt").apply { writeText(scrubbed(Logs.text())) }
        send(context, file, "text/plain", "Youtube Music Spotified log", "Send the log")
    }

    /** The folder shared files go in; an older report is cleared so that they don't pile up. */
    private fun shared(context: Context): File {
        val dir = File(context.cacheDir, "share").apply { mkdirs() }
        dir.listFiles { f -> f.name.startsWith("spotified-report-") }?.forEach { it.delete() }
        return dir
    }

    private fun send(context: Context, file: File, type: String, subject: String, title: String): Intent {
        val uri = FileProvider.getUriForFile(context, "${context.packageName}.files", file)
        val send = Intent(Intent.ACTION_SEND).setType(type)
            .putExtra(Intent.EXTRA_STREAM, uri)
            .putExtra(Intent.EXTRA_SUBJECT, subject)
            .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        return Intent.createChooser(send, title).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
    }
}

/** A number of bytes as a person reads it: 340 MB, 1.2 GB. */
fun sizeText(bytes: Long): String = when {
    bytes >= 1L shl 30 -> String.format(Locale.US, "%.1f GB", bytes.toDouble() / (1L shl 30))
    bytes >= 1L shl 20 -> "${bytes / (1L shl 20)} MB"
    bytes >= 1024 -> "${bytes / 1024} KB"
    else -> "$bytes B"
}
