package dev.darksavci.spotified.car

import androidx.car.app.CarContext
import androidx.car.app.constraints.ConstraintManager
import androidx.car.app.model.Action
import androidx.car.app.model.GridSection
import androidx.car.app.model.Header
import androidx.car.app.model.RowSection
import androidx.car.app.model.SectionedItemTemplate
import androidx.car.app.model.Template
import dev.darksavci.spotified.R
import dev.darksavci.spotified.api.Album
import dev.darksavci.spotified.api.Artist
import dev.darksavci.spotified.api.Track
import dev.darksavci.spotified.api.Item as ApiItem

/**
 * An album, playlist, artist or podcast: its songs to play, Play and Shuffle
 * for the whole, and for an artist their albums, singles and the artists
 * like them.
 */
class CollectionScreen(carContext: CarContext, private val collection: Collection) : Page(carContext) {
    private var songs: List<Track> = emptyList()
    private var subtitle = ""
    private var artist: Artist? = null

    init {
        load {
            when (collection) {
                is Collection.Album -> {
                    val album = graph.api.album(collection.id)
                    songs = album.tracks.map { if (it.artwork.isEmpty()) it.copy(artwork = album.artwork) else it }
                    subtitle = albumLine(album)
                }
                is Collection.Playlist -> {
                    val playlist = graph.api.completePlaylist(collection.id)
                    songs = playlist.tracks
                    subtitle = playlist.owner
                }
                is Collection.Artist -> {
                    val found = graph.api.artist(collection.id)
                    artist = found
                    songs = found.topTracks
                }
                is Collection.Podcast -> {
                    val podcast = graph.api.podcast(collection.id)
                    songs = podcast.episodes
                    subtitle = podcast.author
                }
            }
        }
    }

    override fun onGetTemplate(): Template {
        val template = SectionedItemTemplate.Builder()
            .setHeader(Header.Builder().setStartHeaderAction(Action.BACK).setTitle(collection.title).build())
        if (loading) return template.setLoading(true).build()
        failure?.let { message ->
            return template.addSection(RowSection.Builder().setNoItemsMessage(message).build()).build()
        }
        val origin = collection.title
        val shown = songs.take(limit())
        // An album's songs share its cover; numbers say more than the same picture again.
        val album = collection is Collection.Album
        template.addSection(
            RowSection.Builder()
                .setTitle(if (collection is Collection.Artist) "Popular" else subtitle.ifEmpty { "${songs.size} songs" })
                .setNoItemsMessage("Nothing to play here.")
                .setItems(shown.indices.map { songRow(shown, it, origin, withArt = !album) })
                .build(),
        )
        artist?.let { artist ->
            albums("Albums", artist.albums)?.let(template::addSection)
            albums("Singles", artist.singles)?.let(template::addSection)
            if (artist.related.isNotEmpty()) {
                val related = artist.related.take(limit(ConstraintManager.CONTENT_LIMIT_TYPE_GRID))
                val items = related.map { ApiItem.ArtistItem(it) }
                template.addSection(
                    GridSection.Builder()
                        .setTitle("Fans also like")
                        .setItemSize(GridSection.ITEM_SIZE_MEDIUM)
                        .setItemImageShape(GridSection.ITEM_IMAGE_SHAPE_CIRCLE)
                        .setItems(items.mapNotNull { tile(items, it, origin) })
                        .build(),
                )
            }
        }
        if (songs.isNotEmpty()) {
            template.addAction(fab(R.drawable.ic_play_filled, primary = true) { play(songs, 0, origin) })
            template.addAction(fab(R.drawable.ic_shuffle) { shuffle(songs, origin) })
        }
        return template.build()
    }

    private fun albums(title: String, albums: List<Album>): GridSection? {
        if (albums.isEmpty()) return null
        val items = albums.take(limit(ConstraintManager.CONTENT_LIMIT_TYPE_GRID)).map { ApiItem.AlbumItem(it) }
        return GridSection.Builder()
            .setTitle(title)
            .setItemSize(GridSection.ITEM_SIZE_MEDIUM)
            .setItems(items.mapNotNull { tile(items, it, collection.title) })
            .build()
    }
}
