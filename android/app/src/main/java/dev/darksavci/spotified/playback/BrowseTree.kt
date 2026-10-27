package dev.darksavci.spotified.playback

import android.net.Uri
import android.os.Bundle
import androidx.annotation.OptIn
import androidx.core.net.toUri
import androidx.media3.common.MediaItem
import androidx.media3.common.MediaMetadata
import androidx.media3.common.util.UnstableApi
import dev.darksavci.spotified.api.Api
import dev.darksavci.spotified.api.Item
import dev.darksavci.spotified.api.LibraryKind
import dev.darksavci.spotified.api.SearchFilter
import dev.darksavci.spotified.api.Track
import dev.darksavci.spotified.api.url
import java.util.concurrent.ConcurrentHashMap

/**
 * What Android Auto, and any other media browser, can walk: Home, the
 * library, liked songs and recent plays, each down to its songs.
 *
 * A song's id names the list it is in and its place there,
 * "<list>#<index>", so choosing one plays the whole list from it; Auto hands
 * back only ids, never the extras an item carried. Lists are kept as they
 * were served for that.
 */
@OptIn(UnstableApi::class)
class BrowseTree(private val api: Api, private val artwork: (String?) -> Uri?) {
    private val lists = ConcurrentHashMap<String, List<Track>>()

    val root: MediaItem = folder(ROOT, "Youtube Music Spotified")

    /** The children of [parentId]: folders to open, or songs to play. */
    suspend fun children(parentId: String): List<MediaItem> = when {
        parentId == ROOT -> listOf(
            folder(HOME, "Home", grid = true),
            folder(LIBRARY, "Library", grid = true),
            folder(LIKED, "Liked"),
            folder(RECENT, "Recent"),
        )
        parentId == HOME -> api.home().shelves.mapIndexedNotNull { i, shelf ->
            folder("$SHELF$i", shelf.title.ifEmpty { "More" }, grid = true).takeIf { shelf.items.isNotEmpty() }
        }
        parentId.startsWith(SHELF) -> {
            val shelf = api.home().shelves.getOrNull(parentId.removePrefix(SHELF).toIntOrNull() ?: -1)
            itemsOf(parentId, shelf?.items.orEmpty())
        }
        parentId == LIBRARY -> api.library().sortedByDescending { it.pinned }.mapNotNull { item ->
            val art = item.artwork.url(300)
            when (item.kind) {
                LibraryKind.Playlist -> folder("$PLAYLIST${item.id}", item.title, item.subtitle, art)
                LibraryKind.Album -> folder("$ALBUM${item.id}", item.title, item.subtitle, art)
                LibraryKind.Artist -> folder("$ARTIST${item.id}", item.title, item.subtitle, art)
                LibraryKind.Podcast -> folder("$PODCAST${item.id}", item.title, item.subtitle, art)
            }
        }
        parentId == LIKED -> songs(LIKED, api.liked().tracks)
        parentId == RECENT -> songs(RECENT, api.history())
        parentId.startsWith(PLAYLIST) -> songs(parentId, api.completePlaylist(parentId.removePrefix(PLAYLIST)).tracks)
        parentId.startsWith(ALBUM) -> {
            val album = api.album(parentId.removePrefix(ALBUM))
            songs(parentId, album.tracks.map { if (it.artwork.isEmpty()) it.copy(artwork = album.artwork) else it })
        }
        parentId.startsWith(ARTIST) -> songs(parentId, api.artist(parentId.removePrefix(ARTIST)).topTracks)
        parentId.startsWith(PODCAST) -> songs(parentId, api.podcast(parentId.removePrefix(PODCAST)).episodes)
        else -> emptyList()
    }

    /** Songs found for [query], as the car's search and voice ask. */
    suspend fun search(query: String): List<MediaItem> {
        val results = api.search(query, SearchFilter.Songs)
        val tracks = results.shelves.flatMap { it.items }.filterIsInstance<Item.Song>().map { it.track }.distinctBy { it.id }
        return songs("$SEARCH$query", tracks.take(40))
    }

    /** The list and place a chosen song's id names, to queue the whole list. */
    fun resolve(mediaId: String): Pair<List<Track>, Int>? {
        val at = mediaId.lastIndexOf('#')
        if (at < 0) return null
        val list = lists[mediaId.substring(0, at)] ?: return null
        val index = mediaId.substring(at + 1).toIntOrNull() ?: return null
        return list to index.coerceIn(0, list.size - 1)
    }

    /** The first songs of a search, for "play X" said to the Assistant. */
    suspend fun songsFor(query: String): List<Track> =
        api.search(query, SearchFilter.Songs).shelves.flatMap { it.items }
            .filterIsInstance<Item.Song>().map { it.track }.distinctBy { it.id }.take(25)

    private fun itemsOf(parent: String, items: List<Item>): List<MediaItem> {
        val tracks = items.filterIsInstance<Item.Song>().map { it.track }
        val songItems = songs(parent, tracks)
        val folders = items.mapNotNull { item ->
            when (item) {
                is Item.AlbumItem -> folder("$ALBUM${item.album.id}", item.title, item.album.artists.joinToString { it.name }, item.artwork.url(300))
                is Item.PlaylistItem -> folder("$PLAYLIST${item.playlist.id}", item.title, item.playlist.owner, item.artwork.url(300))
                is Item.ArtistItem -> folder("$ARTIST${item.artist.id}", item.title, "Artist", item.artwork.url(300))
                is Item.PodcastItem -> folder("$PODCAST${item.podcast.id}", item.title, item.podcast.author, item.artwork.url(300))
                else -> null
            }
        }
        return songItems + folders
    }

    private fun songs(list: String, tracks: List<Track>): List<MediaItem> {
        val playable = tracks.filter { it.playable && it.id.isNotEmpty() }
        lists[list] = playable
        return playable.mapIndexed { i, t ->
            val item = CorePlayer.mediaItemOf(t)
            item.buildUpon()
                .setMediaId("$list#$i")
                .setMediaMetadata(item.mediaMetadata.buildUpon().setArtworkUri(artwork(t.artwork.url(300))).build())
                .build()
        }
    }

    private fun folder(id: String, title: String, subtitle: String = "", art: String? = null, grid: Boolean = false): MediaItem {
        val extras = Bundle().apply {
            // Auto shows a folder's children as a grid or a list as asked.
            putInt(CONTENT_STYLE_BROWSABLE, if (grid) STYLE_GRID else STYLE_LIST)
            putInt(CONTENT_STYLE_PLAYABLE, STYLE_LIST)
        }
        return MediaItem.Builder()
            .setMediaId(id)
            .setMediaMetadata(
                MediaMetadata.Builder()
                    .setTitle(title)
                    .setSubtitle(subtitle.ifEmpty { null })
                    .setArtworkUri(artwork(art) ?: art?.toUri())
                    .setIsBrowsable(true)
                    .setIsPlayable(false)
                    .setMediaType(MediaMetadata.MEDIA_TYPE_FOLDER_MIXED)
                    .setExtras(extras)
                    .build(),
            )
            .build()
    }

    companion object {
        const val ROOT = "root"
        const val HOME = "home"
        const val LIBRARY = "library"
        const val LIKED = "liked"
        const val RECENT = "recent"
        const val SHELF = "shelf:"
        const val PLAYLIST = "playlist:"
        const val ALBUM = "album:"
        const val ARTIST = "artist:"
        const val PODCAST = "podcast:"
        const val SEARCH = "search:"

        const val CONTENT_STYLE_BROWSABLE = "android.media.browse.CONTENT_STYLE_BROWSABLE_HINT"
        const val CONTENT_STYLE_PLAYABLE = "android.media.browse.CONTENT_STYLE_PLAYABLE_HINT"
        const val STYLE_LIST = 1
        const val STYLE_GRID = 2
    }
}
