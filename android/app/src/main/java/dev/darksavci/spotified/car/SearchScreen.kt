package dev.darksavci.spotified.car

import androidx.car.app.CarContext
import androidx.car.app.model.Action
import androidx.car.app.model.ItemList
import androidx.car.app.model.Row
import androidx.car.app.model.SearchTemplate
import androidx.car.app.model.Template
import androidx.lifecycle.lifecycleScope
import dev.darksavci.spotified.api.Item
import dev.darksavci.spotified.api.SearchResults
import kotlinx.coroutines.Job
import kotlinx.coroutines.launch

/**
 * Search, typed or said: the top result first, then songs, artists, albums
 * and playlists. A song plays the songs found from there; the rest open.
 */
class SearchScreen(carContext: CarContext, initial: String = "") : Page(carContext) {
    private var query = ""
    private var results: SearchResults? = null
    private var searching: Job? = null

    init {
        if (initial.isNotBlank()) find(initial)
    }

    override fun onGetTemplate(): Template {
        val busy = searching?.isActive == true
        val template = SearchTemplate.Builder(object : SearchTemplate.SearchCallback {
            override fun onSearchSubmitted(searchText: String) = find(searchText)
        })
            .setHeaderAction(Action.BACK)
            .setSearchHint("Songs, artists, albums")
            .setInitialSearchText(query)
            .setShowKeyboardByDefault(query.isEmpty())
            .setLoading(busy)
        if (!busy) {
            val list = ItemList.Builder().setNoItemsMessage(if (query.isEmpty()) "Search for songs, artists and albums." else "Nothing was found for \"$query\".")
            results?.let { found -> rows(found).take(limit()).forEach(list::addItem) }
            template.setItemList(list.build())
        }
        return template.build()
    }

    private fun find(text: String) {
        query = text.trim()
        if (query.isEmpty()) return
        searching?.cancel()
        searching = lifecycleScope.launch {
            results = runCatching { graph.api.search(query) }.getOrNull()
            invalidate()
        }
        invalidate()
    }

    private fun rows(found: SearchResults): List<Row> {
        val items = (listOfNotNull(found.topResult) + found.topResultItems + found.shelves.flatMap { it.items })
            .distinctBy { listOf(it.javaClass.simpleName, it.title, trackOf(it)?.id) }
        val songs = items.mapNotNull(::trackOf)
        return items.map { item ->
            val track = trackOf(item)
            if (track != null) return@map songRow(songs, songs.indexOf(track), query)
            val (kind, collection) = when (item) {
                is Item.AlbumItem -> listOf("Album", item.album.artists.joinToString { it.name }) to Collection.Album(item.album.id, item.title)
                is Item.PlaylistItem -> listOf("Playlist", item.playlist.owner) to Collection.Playlist(item.playlist.id, item.title)
                is Item.ArtistItem -> listOf("Artist") to Collection.Artist(item.artist.id, item.title)
                is Item.PodcastItem -> listOf("Podcast", item.podcast.author) to Collection.Podcast(item.podcast.id, item.title)
                is Item.Song, is Item.Episode -> error("songs are rows above")
            }
            Row.Builder()
                .setTitle(item.title.ifEmpty { "Unknown" })
                .addText(kind.filter { it.isNotEmpty() }.joinToString(" • "))
                .apply { art(item.artwork, 120)?.let { setImage(it, Row.IMAGE_TYPE_LARGE) } }
                .setBrowsable(true)
                .setOnClickListener { open(collection) }
                .build()
        }
    }
}
