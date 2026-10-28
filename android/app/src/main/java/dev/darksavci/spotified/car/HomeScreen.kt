package dev.darksavci.spotified.car

import androidx.car.app.CarContext
import androidx.car.app.constraints.ConstraintManager
import androidx.car.app.model.Action
import androidx.car.app.model.GridItem
import androidx.car.app.model.GridSection
import androidx.car.app.model.Row
import androidx.car.app.model.RowSection
import androidx.car.app.model.SectionedItemTemplate
import androidx.car.app.model.Tab
import androidx.car.app.model.TabContents
import androidx.car.app.model.TabTemplate
import androidx.car.app.model.Template
import androidx.lifecycle.lifecycleScope
import dev.darksavci.spotified.R
import dev.darksavci.spotified.api.ApiError
import dev.darksavci.spotified.api.Item
import dev.darksavci.spotified.api.LibraryItem
import dev.darksavci.spotified.api.LibraryKind
import dev.darksavci.spotified.api.Shelf
import dev.darksavci.spotified.api.Track
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch

/**
 * The car's first screen: tabs for Home, search, the library and liked
 * songs. Each tab loads when it is first opened.
 */
class HomeScreen(carContext: CarContext) : Page(carContext) {
    private var active = HOME

    private var recent: List<Track>? = null
    private var shelves: List<Shelf>? = null
    private var library: List<LibraryItem>? = null
    private var liked: List<Track>? = null
    private var searches: List<String>? = null

    /** What could not load, by tab. */
    private val failed = mutableMapOf<String, String>()

    init {
        fetch(HOME)
        // Continue listening follows what plays.
        lifecycleScope.launch {
            graph.playback.projection.map { it.state.current?.id }.distinctUntilChanged().collect { if (active == HOME) invalidate() }
        }
    }

    override fun onGetTemplate(): Template {
        val tabs = TabTemplate.Builder(object : TabTemplate.TabCallback {
            override fun onTabSelected(tabContentId: String) {
                active = tabContentId
                fetch(tabContentId)
                invalidate()
            }
        }).setHeaderAction(Action.APP_ICON)
        tabs.addTab(tab(HOME, "Home", R.drawable.ic_house))
        tabs.addTab(tab(SEARCH, "Search", R.drawable.ic_search))
        tabs.addTab(tab(LIBRARY, "Library", R.drawable.ic_library))
        tabs.addTab(tab(LIKED, "Liked", R.drawable.ic_heart))
        tabs.setActiveTabContentId(active)
        tabs.setTabContents(TabContents.Builder(content()).build())
        return tabs.build()
    }

    private fun tab(id: String, title: String, icon: Int) =
        Tab.Builder().setContentId(id).setTitle(title).setIcon(icon(icon)).build()

    /** Loads a tab's content the first time it is shown. */
    private fun fetch(tab: String) {
        val needed = when (tab) {
            HOME -> shelves == null
            LIBRARY -> library == null
            LIKED -> liked == null
            SEARCH -> searches == null
            else -> false
        }
        if (!needed) return
        lifecycleScope.launch {
            val outcome = runCatching {
                when (tab) {
                    HOME -> {
                        recent = runCatching { graph.api.history() }.getOrDefault(emptyList())
                        shelves = graph.api.homeShelves()
                    }
                    LIBRARY -> library = graph.api.library()
                    LIKED -> liked = graph.api.liked().tracks
                    SEARCH -> {
                        // The history keeps what was typed on the way to a search too; those go.
                        val all = graph.api.searchHistory().map { it.query.trim() }.filter { it.isNotEmpty() }.distinct()
                        searches = all.filter { q -> all.none { it != q && it.startsWith(q, ignoreCase = true) } }
                    }
                }
            }
            outcome.exceptionOrNull()?.let { failed[tab] = say(it) } ?: failed.remove(tab)
            invalidate()
        }
    }

    private fun say(e: Throwable) = if (e is ApiError.SignedOut) "Sign in on your phone to see this." else e.message ?: "Something went wrong."

    private fun content(): Template = sections(active)

    private fun sections(tab: String): Template {
        val template = SectionedItemTemplate.Builder()
        val playing = graph.playback.projection.value.state.current != null
        failed[tab]?.let { message ->
            template.addSection(RowSection.Builder().setTitle(message).setNoItemsMessage(message).build())
            return template.build()
        }
        when (tab) {
            HOME -> {
                val shelves = shelves ?: return template.setLoading(true).build()
                home(template, shelves)
            }
            LIBRARY -> {
                val library = library ?: return template.setLoading(true).build()
                val byKind = library.sortedByDescending { it.pinned }.groupBy { it.kind }
                for ((kind, title) in KINDS) {
                    val items = byKind[kind].orEmpty().take(limit(ConstraintManager.CONTENT_LIMIT_TYPE_GRID))
                    if (items.isEmpty()) continue
                    val section = GridSection.Builder().setTitle(title).setItemSize(GridSection.ITEM_SIZE_MEDIUM)
                    if (kind == LibraryKind.Artist) section.setItemImageShape(GridSection.ITEM_IMAGE_SHAPE_CIRCLE)
                    template.addSection(section.setItems(items.map(::libraryTile)).build())
                }
            }
            SEARCH -> {
                val searches = searches ?: return template.setLoading(true).build()
                template.addSection(
                    RowSection.Builder()
                        .setTitle("Recent searches")
                        .setNoItemsMessage("Tap search to find songs, artists and albums.")
                        .setItems(
                            searches.take(limit()).map { query ->
                                Row.Builder()
                                    .setTitle(query)
                                    .setImage(icon(R.drawable.ic_search))
                                    .setBrowsable(true)
                                    .setOnClickListener { screenManager.push(SearchScreen(carContext, query)) }
                                    .build()
                            },
                        )
                        .build(),
                )
                template.addAction(fab(R.drawable.ic_search, primary = true) { screenManager.push(SearchScreen(carContext)) })
            }
            LIKED -> {
                val liked = liked ?: return template.setLoading(true).build()
                val songs = liked.take(limit())
                template.addSection(
                    RowSection.Builder()
                        .setTitle("${liked.size} songs")
                        .setNoItemsMessage("Songs you like show up here.")
                        .setItems(songs.indices.map { songRow(songs, it, "Liked") })
                        .build(),
                )
                if (liked.isNotEmpty()) {
                    template.addAction(fab(R.drawable.ic_shuffle, primary = true) { shuffle(liked, "Liked") })
                }
            }
        }
        if (playing) template.addAction(nowPlayingAction())
        return template.build()
    }

    /**
     * Home, short enough to take in at a glance, and every tile starts music
     * at once: what was playing to carry on with, a row of mixes, a row of
     * playlists. Browsing is the Library's and Search's.
     */
    private fun home(template: SectionedItemTemplate.Builder, shelves: List<Shelf>) {
        continueRow()?.let { template.addSection(RowSection.Builder().setTitle("Continue listening").addItem(it).build()) }

        // Shelves mostly of songs are mixes; the rest are playlists and albums.
        val (songShelves, others) = shelves.filter { it.items.isNotEmpty() }
            .partition { shelf -> shelf.items.count { trackOf(it) != null } * 2 > shelf.items.size }
        val mixes = songShelves.map { it.title.ifEmpty { "Songs" } to it.items.mapNotNull(::trackOf) }.filter { it.second.isNotEmpty() }
        // Each mix shows a cover none of the others shows.
        val used = mutableSetOf<String>()
        val mixTiles = mixes.take(ROW).map { (title, songs) ->
            val cover = songs.firstOrNull { it.album?.id !in used || it.album?.id.isNullOrEmpty() } ?: songs.first()
            cover.album?.id?.let(used::add)
            mixTile(title, songs, cover)
        }
        if (mixTiles.isNotEmpty()) template.addSection(grid("Your mixes", mixTiles, GridSection.ITEM_SIZE_LARGE))

        val playlists = others.flatMap { it.items }
            .filter { it is Item.PlaylistItem || it is Item.AlbumItem }
            .distinctBy { it.title }
            .take(ROW)
        if (playlists.isNotEmpty()) {
            template.addSection(grid("Playlists for you", playlists.map(::playTile), GridSection.ITEM_SIZE_LARGE))
        }
    }

    /** What was playing, or else what played last, as a full-width card that carries on. */
    private fun continueRow(): Row? {
        val state = graph.playback.projection.value.state
        val current = state.current
        val track = current ?: recent.orEmpty().firstOrNull() ?: return null
        val carryOn = {
            if (current != null) {
                graph.playback.resume()
                showNowPlaying(screenManager, carContext)
            } else {
                play(recent.orEmpty(), 0, "Recently played")
            }
        }
        val from = if (current != null) state.queue.origin else "Recently played"
        return Row.Builder()
            .setTitle(track.title)
            .addText(listOf(track.artistNames, from).filter { it.isNotEmpty() }.joinToString(" • "))
            .setImage(art(track.artwork, 300) ?: icon(R.drawable.ic_music), Row.IMAGE_TYPE_LARGE)
            .addAction(Action.Builder().setIcon(icon(R.drawable.ic_play_filled)).setOnClickListener(carryOn).build())
            .setOnClickListener(carryOn)
            .build()
    }

    /** A shelf of songs as one tile that plays it: a cover of its own and who is on it. */
    private fun mixTile(title: String, songs: List<Track>, cover: Track): GridItem {
        val artists = songs.flatMap { it.artists }.map { it.name }.filter { it.isNotEmpty() }.distinct().take(2)
        return GridItem.Builder()
            .setTitle(title)
            .setText(artists.joinToString(", ").ifEmpty { "${songs.size} songs" })
            .setImage(art(cover.artwork) ?: icon(R.drawable.ic_music), GridItem.IMAGE_TYPE_LARGE)
            .setOnClickListener { play(songs, 0, title) }
            .build()
    }

    /** A playlist or album that starts playing when tapped. */
    private fun playTile(item: Item): GridItem {
        val subtitle = when (item) {
            is Item.PlaylistItem -> item.playlist.owner
            is Item.AlbumItem -> albumLine(item.album)
            else -> ""
        }
        return GridItem.Builder()
            .setTitle(item.title.ifEmpty { "Unknown" })
            .apply { if (subtitle.isNotEmpty()) setText(subtitle) }
            .setImage(art(item.artwork) ?: icon(R.drawable.ic_music), GridItem.IMAGE_TYPE_LARGE)
            .setOnClickListener {
                lifecycleScope.launch {
                    val songs = runCatching {
                        when (item) {
                            is Item.PlaylistItem -> graph.api.completePlaylist(item.playlist.id).tracks
                            is Item.AlbumItem -> graph.api.album(item.album.id).let { a -> a.tracks.map { if (it.artwork.isEmpty()) it.copy(artwork = a.artwork) else it } }
                            else -> emptyList()
                        }
                    }.getOrDefault(emptyList())
                    play(songs, 0, item.title)
                }
            }
            .build()
    }

    private fun grid(title: String, tiles: List<GridItem>, size: Int = GridSection.ITEM_SIZE_MEDIUM, round: Boolean = false) =
        GridSection.Builder().setTitle(title).setItemSize(size)
            .apply { if (round) setItemImageShape(GridSection.ITEM_IMAGE_SHAPE_CIRCLE) }
            .setItems(tiles).build()

    private fun libraryTile(item: LibraryItem) = GridItem.Builder()
        .setTitle(item.title.ifEmpty { "Unknown" })
        .apply { if (item.subtitle.isNotEmpty()) setText(item.subtitle) }
        .setImage(art(item.artwork) ?: icon(R.drawable.ic_music), GridItem.IMAGE_TYPE_LARGE)
        .setOnClickListener {
            open(
                when (item.kind) {
                    LibraryKind.Playlist -> Collection.Playlist(item.id, item.title)
                    LibraryKind.Album -> Collection.Album(item.id, item.title)
                    LibraryKind.Artist -> Collection.Artist(item.id, item.title)
                    LibraryKind.Podcast -> Collection.Podcast(item.id, item.title)
                },
            )
        }
        .build()

    companion object {
        private const val HOME = "home"
        private const val SEARCH = "search"
        private const val LIBRARY = "library"
        private const val LIKED = "liked"
        /** One row of large tiles. */
        private const val ROW = 5
        private val KINDS = listOf(
            LibraryKind.Playlist to "Playlists",
            LibraryKind.Album to "Albums",
            LibraryKind.Artist to "Artists",
            LibraryKind.Podcast to "Podcasts",
        )
    }
}
