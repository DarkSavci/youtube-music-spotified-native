package dev.darksavci.spotified.data

import android.content.Context
import java.io.File

/**
 * What changed in each release, read from the CHANGELOG.md the build was made
 * with, which the build copies into the app's assets. Nothing is fetched. A port
 * of crates/app/src/changelog.rs.
 */
object Changelog {
    data class Group(val title: String, val items: List<String>)

    data class Release(val version: String, val date: String, val groups: List<Group>, val note: String)

    /** Reads the file: "## version — date" starts a release, "### title" a group within it, and "- text" a change within that. */
    fun parse(text: String): List<Release> {
        class Building(val version: String, val date: String) {
            val groups = mutableListOf<Pair<String, MutableList<String>>>()
            var note = ""
        }
        val releases = mutableListOf<Building>()
        for (line in text.lineSequence().map { it.trim() }) {
            if (line.startsWith("## ")) {
                val heading = line.removePrefix("## ")
                releases += Building(heading.substringBefore(" — ").trim(), heading.substringAfter(" — ", "").trim())
                continue
            }
            // Whatever comes before the first release is the file's own heading.
            val release = releases.lastOrNull() ?: continue
            when {
                line.startsWith("### ") -> release.groups += line.removePrefix("### ") to mutableListOf()
                line.startsWith("- ") -> release.groups.lastOrNull()?.second?.add(line.removePrefix("- "))
                line.isNotEmpty() && release.groups.isEmpty() -> release.note = line
            }
        }
        return releases.map { r -> Release(r.version, r.date, r.groups.map { Group(plainText(it.first), it.second.map(::plainText)) }, plainText(r.note)) }
    }

    /**
     * A line of the file as words to read: its emphasis, code marks and link
     * addresses are the file's own markup. A mark only counts when closed, so a
     * lone star or underscore in a name stays.
     */
    fun plainText(line: String): String = line
        .replace(LINK) { it.groupValues[1] }
        .replace(CODE) { it.groupValues[1] }
        .replace(BOLD) { it.groupValues[2] }
        .replace(STAR) { it.groupValues[1] }
        .replace(UNDERSCORE) { it.groupValues[1] }

    private val LINK = Regex("""\[([^\]]+)]\([^)]*\)""")
    private val CODE = Regex("`([^`]+)`")
    private val BOLD = Regex("""(\*\*|__)(.+?)\1""")
    private val STAR = Regex("""\*(\S(?:.*?\S)?)\*""")
    private val UNDERSCORE = Regex("""(?<![\w])_(\S(?:.*?\S)?)_(?![\w])""")

    /** The releases newer than [seen], newest first. */
    fun since(releases: List<Release>, seen: String): List<Release> = releases.filter { Updater.newer(it.version, seen) }

    /**
     * What to show now that [current] is running, given the version [last] seen
     * here. A new install shows nothing, and neither does the same version again.
     * With no record but an update, as on the first version that keeps one, it
     * shows the release itself.
     */
    fun toShow(releases: List<Release>, last: String?, current: String, freshInstall: Boolean): List<Release> = when {
        last == null && freshInstall -> emptyList()
        last == null -> releases.filter { it.version == current }
        last == current -> emptyList()
        else -> since(releases, last).filter { !Updater.newer(it.version, current) }
    }

    fun read(context: Context): List<Release> =
        runCatching { context.assets.open("CHANGELOG.md").bufferedReader().use { parse(it.readText()) } }.getOrDefault(emptyList())

    private fun seenFile(context: Context) = File(context.filesDir, "last-version.txt")

    fun lastSeen(context: Context): String? = seenFile(context).takeIf { it.isFile }?.readText()?.trim()?.ifEmpty { null }

    fun markSeen(context: Context, version: String) {
        runCatching { seenFile(context).writeText(version) }
    }
}
