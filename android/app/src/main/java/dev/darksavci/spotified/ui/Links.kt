package dev.darksavci.spotified.ui

import android.net.Uri
import dev.darksavci.spotified.together.Invite
import dev.darksavci.spotified.together.readInvite

/**
 * Reads a link into what it opens: a YouTube Music or YouTube address, the
 * app's own shortcut addresses, or another service's link, which search
 * matches on YouTube Music. Anything else is not a link.
 *
 * Every id is checked against the alphabet it is written in, so a crafted
 * link can only ever name a song, album, artist or playlist.
 */
object Links {
    sealed interface Target {
        data class Song(val id: String) : Target
        data class Album(val id: String) : Target
        data class Artist(val id: String) : Target
        data class Playlist(val id: String) : Target
        data class Page(val route: String) : Target
        data class Elsewhere(val url: String) : Target

        /** An invitation to a Listen Together room. */
        data class Room(val invite: Invite) : Target

        /** A link of the app's own that cannot be used, and why. */
        data class Broken(val why: String) : Target
    }

    private val VIDEO = Regex("^[A-Za-z0-9_-]{11}$")
    private val ID = Regex("^[A-Za-z0-9_-]{2,64}$")
    private val ELSEWHERE = setOf(
        "open.spotify.com", "music.apple.com", "itunes.apple.com", "www.deezer.com", "deezer.com",
        "deezer.page.link", "link.deezer.com", "soundcloud.com", "on.soundcloud.com",
    )

    fun read(text: String): Target? {
        val found = Regex("(https?://|spotified://|spotify:)\\S+").find(text)?.value ?: return null
        if (found.startsWith("spotify:")) return Target.Elsewhere(found)
        val uri = Uri.parse(found)
        val host = uri.host?.lowercase() ?: return null
        if (uri.scheme == "spotified") {
            return when (host) {
                "search" -> Target.Page(Routes.SEARCH)
                "liked" -> Target.Page(Routes.LIKED)
                "downloads" -> Target.Page(Routes.DOWNLOADS)
                "shuffle-liked" -> Target.Page(Routes.LIKED + "?shuffle=1")
                "join" -> readInvite(found).fold({ Target.Room(it) }, { Target.Broken(it.message.orEmpty()) })
                else -> null
            }
        }
        if (host in ELSEWHERE) return Target.Elsewhere(found)
        val path = uri.pathSegments
        return when (host) {
            "youtu.be" -> path.firstOrNull()?.takeIf { VIDEO.matches(it) }?.let(Target::Song)
            "music.youtube.com", "www.youtube.com", "youtube.com", "m.youtube.com" -> when (path.firstOrNull()) {
                "watch" -> uri.getQueryParameter("v")?.takeIf { VIDEO.matches(it) }?.let(Target::Song)
                    ?: uri.getQueryParameter("list")?.takeIf { ID.matches(it) }?.let { Target.Playlist(playlistId(it)) }
                "playlist" -> uri.getQueryParameter("list")?.takeIf { ID.matches(it) }?.let { Target.Playlist(playlistId(it)) }
                "browse" -> path.getOrNull(1)?.takeIf { ID.matches(it) }?.let { id ->
                    if (id.startsWith("MPRE")) Target.Album(id) else if (id.startsWith("VL")) Target.Playlist(id) else Target.Artist(id)
                }
                "channel" -> path.getOrNull(1)?.takeIf { ID.matches(it) }?.let(Target::Artist)
                "shorts", "embed", "live" -> path.getOrNull(1)?.takeIf { VIDEO.matches(it) }?.let(Target::Song)
                else -> null
            }
            else -> null
        }
    }

    /** The core names a playlist by its list id, without the browse id's "VL". */
    private fun playlistId(list: String) = list.removePrefix("VL")
}
