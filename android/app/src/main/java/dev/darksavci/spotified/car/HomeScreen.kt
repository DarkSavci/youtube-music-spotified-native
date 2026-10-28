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
                var room = limit(ConstraintManager.CONTENT_LIMIT_TYPE_GRID).coerceAtLeast(SHELF_ITEMS)
                recent.orEmpty().take(SHELF_ITEMS).takeIf { it.isNotEmpty() }?.let { songs ->
                    val items = songs.map { Item.Song(it) }
                    template.addSection(grid("Recently played", items.mapNotNull { tile(items, it, "Recently played") }))
                    room -= songs.size
                }
                for (shelf in shelves) {
                    if (room <= 0) break
                    val items = shelf.items.take(minOf(SHELF_ITEMS, room))
                    val tiles = items.mapNotNull { tile(items, it, shelf.title) }
                    if (tiles.isEmpty()) continue
                    template.addSection(grid(shelf.title.ifEmpty { "More" }, tiles))
                    room -= tiles.size
                }
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

    private fun grid(title: String, tiles: List<GridItem>) =
        GridSection.Builder().setTitle(title).setItemSize(GridSection.ITEM_SIZE_MEDIUM).setItems(tiles).build()

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
        private const val SHELF_ITEMS = 10
        private val KINDS = listOf(
            LibraryKind.Playlist to "Playlists",
            LibraryKind.Album to "Albums",
            LibraryKind.Artist to "Artists",
            LibraryKind.Podcast to "Podcasts",
        )
    }
}
