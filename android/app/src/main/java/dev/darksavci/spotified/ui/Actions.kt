package dev.darksavci.spotified.ui

import android.content.Context
import android.content.Intent
import android.net.Uri
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.navigation.NavHostController
import dev.darksavci.spotified.Graph
import dev.darksavci.spotified.api.Album
import dev.darksavci.spotified.api.Item
import dev.darksavci.spotified.api.Playlist
import dev.darksavci.spotified.api.Track
import kotlinx.coroutines.launch

/** Routes, as the navigation graph names them. Ids are encoded when built. */
object Routes {
    const val HOME = "home"
    const val SEARCH = "search"
    const val SEARCH_ROUTE = "search?q={q}"
    const val LIBRARY = "library"
    const val ALBUM = "album/{id}"
    const val ARTIST = "artist/{id}"
    const val PLAYLIST = "playlist/{id}"
    const val PODCAST = "podcast/{id}"
    const val BROWSE = "browse/{id}?params={params}&title={title}"
    const val LIKED = "liked"
    const val LIKED_ROUTE = "liked?shuffle={shuffle}"
    const val HISTORY = "history"
    const val STATS = "stats"
    const val SETTINGS = "settings"
    const val SIGN_IN = "sign-in"
    const val EQUALIZER = "equalizer"
    const val DOWNLOADS = "downloads"
    const val LOCAL = "local"
    const val TOGETHER = "together"
    const val IMPORT = "import"
    const val RECAP = "recap"

    fun album(id: String) = "album/${Uri.encode(id)}"
    fun artist(id: String) = "artist/${Uri.encode(id)}"
    fun playlist(id: String) = "playlist/${Uri.encode(id)}"
    fun podcast(id: String) = "podcast/${Uri.encode(id)}"
    fun browse(id: String, params: String = "", title: String = "") =
        "browse/${Uri.encode(id)}?params=${Uri.encode(params)}&title=${Uri.encode(title)}"
}

/** What a menu was opened on. */
sealed interface MenuTarget {
    /** A song; [playlist] and [itemId] when it sits in an editable playlist, to remove it. */
    data class Song(val track: Track, val playlist: Playlist? = null, val queueIndex: Int? = null) : MenuTarget
    data class Entity(val item: Item) : MenuTarget
    data class AddTo(val tracks: List<Track>) : MenuTarget
    object SleepTimer : MenuTarget
    object Speed : MenuTarget
    object Queue : MenuTarget
}

/**
 * What a screen can do, in one place, so opening, playing and menus behave
 * the same everywhere.
 */
class Actions(val graph: Graph, val nav: NavHostController, private val context: Context) {
    var menu by mutableStateOf<MenuTarget?>(null)
    var nowPlayingOpen by mutableStateOf(false)
    var lyricsOpen by mutableStateOf(false)

    val playback get() = graph.playback

    fun go(route: String) {
        nowPlayingOpen = false
        lyricsOpen = false
        nav.navigate(route) { launchSingleTop = true }
    }

    fun open(item: Item) = when (item) {
        is Item.Song -> play(listOf(item.track), 0, item.track.title)
        is Item.Episode -> play(listOf(item.track), 0, item.track.title)
        is Item.AlbumItem -> go(Routes.album(item.album.id))
        is Item.ArtistItem -> go(Routes.artist(item.artist.id))
        is Item.PlaylistItem -> go(Routes.playlist(item.playlist.id))
        is Item.PodcastItem -> go(Routes.podcast(item.podcast.id))
    }

    fun openAlbumOf(track: Track) {
        track.album?.id?.takeIf { it.isNotEmpty() }?.let { go(Routes.album(it)) }
    }

    fun openArtistOf(track: Track) {
        track.artists.firstOrNull { it.id.isNotEmpty() }?.let { go(Routes.artist(it.id)) }
    }

    fun play(tracks: List<Track>, index: Int, origin: String) = playback.play(tracks, index, origin)

    fun playAlbum(album: Album, index: Int = 0) =
        play(album.tracks.map { if (it.artwork.isEmpty()) it.copy(artwork = album.artwork) else it }, index, album.title)

    fun menuFor(track: Track, playlist: Playlist? = null, queueIndex: Int? = null) {
        menu = MenuTarget.Song(track, playlist, queueIndex)
    }

    fun menuFor(item: Item) {
        menu = when (item) {
            is Item.Song -> MenuTarget.Song(item.track)
            is Item.Episode -> MenuTarget.Song(item.track)
            else -> MenuTarget.Entity(item)
        }
    }

    fun addTo(tracks: List<Track>) {
        menu = MenuTarget.AddTo(tracks)
    }

    /** Shares the YouTube Music address, through the system's share sheet. */
    fun share(title: String, url: String) {
        val send = Intent(Intent.ACTION_SEND).setType("text/plain")
            .putExtra(Intent.EXTRA_SUBJECT, title)
            .putExtra(Intent.EXTRA_TEXT, url)
        context.startActivity(Intent.createChooser(send, "Share").addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
    }

    fun say(message: String) = playback.say(message)

    fun launch(block: suspend () -> Unit) {
        graph.scope.launch { runCatching { block() }.onFailure { say(it.message ?: "That did not work.") } }
    }

    companion object {
        fun songLink(id: String) = "https://music.youtube.com/watch?v=$id"
        fun albumLink(id: String) = "https://music.youtube.com/browse/$id"
        fun playlistLink(id: String) = "https://music.youtube.com/playlist?list=${id.removePrefix("VL")}"
        fun artistLink(id: String) = "https://music.youtube.com/channel/$id"
    }
}

val LocalActions = staticCompositionLocalOf<Actions> { error("no actions") }
