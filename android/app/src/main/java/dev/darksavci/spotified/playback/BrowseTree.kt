package dev.darksavci.spotified.playback

import android.net.Uri
import android.os.Bundle
import androidx.annotation.OptIn
import androidx.core.net.toUri
import androidx.media3.common.MediaItem
import androidx.media3.common.MediaMetadata
import androidx.media3.common.util.UnstableApi
import androidx.media3.session.MediaConstants
import dev.darksavci.spotified.BuildConfig
import dev.darksavci.spotified.api.Album
import dev.darksavci.spotified.api.Api
import dev.darksavci.spotified.api.Item
import dev.darksavci.spotified.api.LibraryItem
import dev.darksavci.spotified.api.LibraryKind
import dev.darksavci.spotified.api.SearchFilter
import dev.darksavci.spotified.api.Shelf
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
            folder(HOME, "Home", grid = true, songsAsGrid = true, icon = icon("ic_house")),
            folder(LIBRARY, "Library", grid = true, icon = icon("ic_library")),
            folder(LIKED, "Liked", icon = icon("ic_heart")),
            folder(RECENT, "Recent", icon = icon("ic_clock")),
        )
        // Home as the phone shows it: each shelf's first items under its title.
        parentId == HOME -> loadHome().withIndex().flatMap { (i, shelf) ->
            itemsOf("$SHELF$i", shelf.items.take(SHELF_ITEMS), group = shelf.title.ifEmpty { "More" })
        }
        parentId.startsWith(SHELF) -> {
            val shelf = (home ?: loadHome()).getOrNull(parentId.removePrefix(SHELF).toIntOrNull() ?: -1)
            itemsOf(parentId, shelf?.items.orEmpty())
        }
        // The library under a heading for each kind, the pinned first in each.
        parentId == LIBRARY -> api.library()
            .sortedWith(compareBy<LibraryItem> { LIBRARY_KINDS.indexOf(it.kind) }.thenByDescending { it.pinned })
            .map { item ->
                val (id, group) = when (item.kind) {
                    LibraryKind.Playlist -> "$PLAYLIST${item.id}" to "Playlists"
                    LibraryKind.Album -> "$ALBUM${item.id}" to "Albums"
                    LibraryKind.Artist -> "$ARTIST${item.id}" to "Artists"
                    LibraryKind.Podcast -> "$PODCAST${item.id}" to "Podcasts"
                }
                folder(id, item.title, item.subtitle, item.artwork.url(300), group = group)
            }
        parentId == LIKED -> songs(LIKED, api.liked().tracks).let { songs ->
            if (songs.size > 1) listOf(shufflePlay(LIKED)) + songs else songs
        }
        parentId == RECENT -> songs(RECENT, api.history())
        parentId.startsWith(PLAYLIST) -> songs(parentId, api.completePlaylist(parentId.removePrefix(PLAYLIST)).tracks)
        parentId.startsWith(ALBUM) -> {
            val album = api.album(parentId.removePrefix(ALBUM))
            songs(parentId, album.tracks.map { if (it.artwork.isEmpty()) it.copy(artwork = album.artwork) else it })
        }
        parentId.startsWith(ARTIST) -> {
            val artist = api.artist(parentId.removePrefix(ARTIST))
            songs(parentId, artist.topTracks).map { it.grouped("Popular") } +
                artist.albums.map { albumFolder(it, "Albums") } +
                artist.singles.map { albumFolder(it, "Singles") }
        }
        parentId.startsWith(PODCAST) -> songs(parentId, api.podcast(parentId.removePrefix(PODCAST)).episodes)
        else -> emptyList()
    }

    /** Home's shelves as the car was last shown them, so a shelf's number finds the same one. */
    @Volatile private var home: List<Shelf>? = null

    private suspend fun loadHome(): List<Shelf> = api.homeShelves().also { home = it }

    /** What [query] finds, as the car's search asks: the top result, then each shelf under its title. */
    suspend fun search(query: String): List<MediaItem> {
        val results = api.search(query)
        val top = (listOfNotNull(results.topResult) + results.topResultItems).distinctBy { it.title }
        return itemsOf("${SEARCH}top:$query", top, group = "Top result") +
            results.shelves.withIndex().flatMap { (i, shelf) ->
                itemsOf("$SEARCH$i:$query", shelf.items.take(SHELF_ITEMS), group = shelf.title.ifEmpty { "More" })
            }
    }

    /** Whether [mediaId] is a list's Shuffle play. */
    fun isShufflePlay(mediaId: String) = mediaId.endsWith(SHUFFLE)

    /** The list a Shuffle play names, in a shuffled order. */
    fun shuffled(mediaId: String): List<Track>? = lists[mediaId.removeSuffix(SHUFFLE)]?.shuffled()

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

    /**
     * [items] in their order, songs to play and the rest to open, under
     * [group]'s heading when there is one. The songs make the list a chosen
     * one plays from.
     */
    private fun itemsOf(parent: String, items: List<Item>, group: String? = null): List<MediaItem> {
        val songItems = songs(parent, items.mapNotNull(::trackOf))
        val playable = lists[parent].orEmpty()
        var next = 0
        return items.mapNotNull { item ->
            when (item) {
                // songs() drops what cannot play; the rest keep their order.
                is Item.Song, is Item.Episode ->
                    if (next < playable.size && playable[next] === trackOf(item)) songItems[next++].grouped(group) else null
                is Item.AlbumItem -> albumFolder(item.album, group)
                is Item.PlaylistItem -> folder("$PLAYLIST${item.playlist.id}", item.title, item.playlist.owner, item.artwork.url(300), group = group)
                is Item.ArtistItem -> folder("$ARTIST${item.artist.id}", item.title, "Artist", item.artwork.url(300), group = group)
                is Item.PodcastItem -> folder("$PODCAST${item.podcast.id}", item.title, item.podcast.author, item.artwork.url(300), group = group)
            }
        }
    }

    private fun albumFolder(album: Album, group: String?): MediaItem {
        val by = album.artists.joinToString { it.name }
        val subtitle = listOf(album.year, by).filter { it.isNotEmpty() }.joinToString(" • ")
        return folder("$ALBUM${album.id}", album.title, subtitle, album.artwork.url(300), group = group)
    }

    /** A list's first item: plays the whole list in a shuffled order. */
    private fun shufflePlay(list: String): MediaItem = MediaItem.Builder()
        .setMediaId("$list$SHUFFLE")
        .setMediaMetadata(
            MediaMetadata.Builder()
                .setTitle("Shuffle play")
                .setArtworkUri(icon("ic_shuffle"))
                .setIsBrowsable(false)
                .setIsPlayable(true)
                .setMediaType(MediaMetadata.MEDIA_TYPE_PLAYLIST)
                .build(),
        )
        .build()

    /** One of the app's own icons, as the car loads a tab's or an action's picture. */
    private fun icon(name: String): Uri = "android.resource://${BuildConfig.APPLICATION_ID}/drawable/$name".toUri()

    private fun trackOf(item: Item): Track? = when (item) {
        is Item.Song -> item.track
        is Item.Episode -> item.track
        else -> null
    }

    /** This item under [group]'s heading: Auto puts neighbours that share one together. */
    private fun MediaItem.grouped(group: String?): MediaItem {
        if (group == null) return this
        val extras = Bundle(mediaMetadata.extras ?: Bundle()).apply { putString(MediaConstants.EXTRAS_KEY_CONTENT_STYLE_GROUP_TITLE, group) }
        return buildUpon().setMediaMetadata(mediaMetadata.buildUpon().setExtras(extras).build()).build()
    }

    private fun songs(list: String, tracks: List<Track>): List<MediaItem> {
        val playable = tracks.filter { it.playable && it.id.isNotEmpty() }
        lists[list] = playable
        return playable.mapIndexed { i, t ->
            val item = CorePlayer.mediaItemOf(t)
            val extras = Bundle(item.mediaMetadata.extras ?: Bundle()).apply {
                if (t.explicit) putLong(MediaConstants.EXTRAS_KEY_IS_EXPLICIT, MediaConstants.EXTRAS_VALUE_ATTRIBUTE_PRESENT)
            }
            item.buildUpon()
                .setMediaId("$list#$i")
                .setMediaMetadata(item.mediaMetadata.buildUpon().setArtworkUri(artwork(t.artwork.url(300))).setExtras(extras).build())
                .build()
        }
    }

    private fun folder(
        id: String, title: String, subtitle: String = "", art: String? = null,
        grid: Boolean = false, songsAsGrid: Boolean = false, group: String? = null, icon: Uri? = null,
    ): MediaItem {
        val extras = Bundle().apply {
            // Auto shows a folder's children as a grid or a list as asked.
            putInt(CONTENT_STYLE_BROWSABLE, if (grid) STYLE_GRID else STYLE_LIST)
            putInt(CONTENT_STYLE_PLAYABLE, if (songsAsGrid) STYLE_GRID else STYLE_LIST)
            if (group != null) putString(MediaConstants.EXTRAS_KEY_CONTENT_STYLE_GROUP_TITLE, group)
        }
        return MediaItem.Builder()
            .setMediaId(id)
            .setMediaMetadata(
                MediaMetadata.Builder()
                    .setTitle(title)
                    .setSubtitle(subtitle.ifEmpty { null })
                    .setArtworkUri(icon ?: artwork(art) ?: art?.toUri())
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
        private const val SHELF_ITEMS = 10
        private val LIBRARY_KINDS = listOf(LibraryKind.Playlist, LibraryKind.Album, LibraryKind.Artist, LibraryKind.Podcast)
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
        private const val SHUFFLE = "#shuffle"

        const val CONTENT_STYLE_BROWSABLE = "android.media.browse.CONTENT_STYLE_BROWSABLE_HINT"
        const val CONTENT_STYLE_PLAYABLE = "android.media.browse.CONTENT_STYLE_PLAYABLE_HINT"
        const val STYLE_LIST = 1
        const val STYLE_GRID = 2
    }
}
