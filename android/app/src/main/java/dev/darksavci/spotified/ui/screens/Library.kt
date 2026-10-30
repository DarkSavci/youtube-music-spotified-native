package dev.darksavci.spotified.ui.screens

import dev.darksavci.spotified.ui.components.SearchField
import dev.darksavci.spotified.ui.components.SearchCancel
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.lazy.grid.rememberLazyGridState
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import dev.darksavci.spotified.R
import dev.darksavci.spotified.api.Album
import dev.darksavci.spotified.api.Artist
import dev.darksavci.spotified.api.Item
import dev.darksavci.spotified.api.LibraryItem
import dev.darksavci.spotified.api.LibraryKind
import dev.darksavci.spotified.api.Playlist
import dev.darksavci.spotified.api.Podcast
import dev.darksavci.spotified.ui.Actions
import dev.darksavci.spotified.ui.Change
import dev.darksavci.spotified.ui.LocalActions
import dev.darksavci.spotified.ui.MenuTarget
import dev.darksavci.spotified.ui.ProfileButton
import dev.darksavci.spotified.ui.Routes
import dev.darksavci.spotified.ui.components.Chip
import dev.darksavci.spotified.ui.components.ChoiceSheet
import dev.darksavci.spotified.ui.components.ConfirmDialog
import dev.darksavci.spotified.ui.components.CoverCard
import dev.darksavci.spotified.ui.components.Empty
import dev.darksavci.spotified.ui.components.EntityRow
import dev.darksavci.spotified.ui.components.Glyph
import dev.darksavci.spotified.ui.components.GlyphButton
import dev.darksavci.spotified.ui.components.Load
import dev.darksavci.spotified.ui.components.PageAction
import dev.darksavci.spotified.ui.components.PageMenu
import dev.darksavci.spotified.ui.components.Problem
import dev.darksavci.spotified.ui.components.Refreshable
import dev.darksavci.spotified.ui.components.ShortcutTile
import dev.darksavci.spotified.ui.components.SkeletonList
import dev.darksavci.spotified.ui.components.SmallProblem
import dev.darksavci.spotified.ui.components.TabScroll
import dev.darksavci.spotified.ui.components.TextDialog
import dev.darksavci.spotified.ui.components.predictiveBack
import dev.darksavci.spotified.ui.components.rememberLoader
import dev.darksavci.spotified.ui.components.tapRow
import dev.darksavci.spotified.ui.theme.LocalPalette
import dev.darksavci.spotified.ui.theme.Type

@Composable
fun LibraryScreen() {
    val actions = LocalActions.current
    val api = actions.graph.api
    val prefs = PagePrefs.of(LocalContext.current)
    val p = LocalPalette.current
    val library = rememberLoader("library", follows = setOf(Change.Library)) { api.library() }
    val state by library.state.collectAsStateWithLifecycle()
    val folders = rememberLoader("folders", follows = setOf(Change.Folders)) { api.folders() }
    val foldersState by folders.state.collectAsStateWithLifecycle()
    val folderList = (foldersState as? Load.Ready)?.value.orEmpty()
    var kind by rememberSaveable { mutableStateOf<LibraryKind?>(null) }
    var folder by rememberSaveable { mutableStateOf<String?>(null) }
    var query by rememberSaveable { mutableStateOf("") }
    var searching by rememberSaveable { mutableStateOf(false) }
    var sorting by remember { mutableStateOf(false) }
    var creating by remember { mutableStateOf(false) }
    var naming by remember { mutableStateOf(false) }
    var deleting by remember { mutableStateOf(false) }
    val grid = prefs.libraryGrid
    // As many covers as fit, at least three, wide enough to fill the row between its gaps.
    val screen = LocalConfiguration.current.screenWidthDp.dp
    val gridColumns = (((screen - GRID_EDGE * 2 + GRID_GAP) / (GRID_CELL + GRID_GAP)).toInt()).coerceAtLeast(3)
    val cell = (screen - GRID_EDGE * 2 - GRID_GAP * (gridColumns - 1)) / gridColumns
    val sort = prefs.librarySort
    val open = folderList.firstOrNull { it.id == folder }
    val list = rememberLazyGridState()
    val field = remember { FocusRequester() }
    TabScroll(Routes.LIBRARY, list)
    // A folder deleted here or elsewhere stops being where the page is.
    LaunchedEffect(foldersState, folder) {
        if (folder != null && foldersState is Load.Ready && open == null) folder = null
    }
    LaunchedEffect(searching) { if (searching) runCatching { field.requestFocus() } }
    // Back leaves the search first, then the folder, then the tab.
    predictiveBack(searching || folder != null) {
        if (searching) { searching = false; query = "" } else folder = null
    }

    Column(Modifier.windowInsetsPadding(WindowInsets.statusBars)) {
        Row(Modifier.fillMaxWidth().padding(start = if (folder == null) 16.dp else 4.dp, end = 4.dp, top = 8.dp), verticalAlignment = Alignment.CenterVertically) {
            if (folder == null) {
                ProfileButton(actions, Modifier.padding(end = 12.dp))
            } else {
                GlyphButton(R.drawable.ic_chevron_left, "Out of this folder", { folder = null })
            }
            if (searching) {
                SearchField(query, { query = it }, Modifier.weight(1f), placeholder = "Search your library", focusRequester = field)
                SearchCancel({ searching = false; query = "" })
            } else {
                Text(open?.name ?: "Your Library", style = Type.title, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f))
                GlyphButton(R.drawable.ic_search, "Search your library", { searching = true })
                if (open != null) GlyphButton(R.drawable.ic_trash_2, "Delete this folder", { deleting = true })
                else GlyphButton(R.drawable.ic_plus, "New playlist or folder", { creating = true })
            }
        }
        LazyRow(contentPadding = PaddingValues(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            items(LibraryKind.entries) { k ->
                Chip(k.name + "s", kind == k, { kind = if (kind == k) null else k })
            }
            if (folder == null) {
                items(folderList, key = { "folder:${it.id}" }) { f ->
                    Chip(f.name, false, { folder = f.id }, role = Role.Button, description = "Folder ${f.name}", icon = R.drawable.ic_folder)
                }
            }
        }
        if (foldersState is Load.Failed) SmallProblem("Couldn't load your folders.", { folders.reload() })
        Row(Modifier.fillMaxWidth().padding(start = 16.dp, end = 8.dp), verticalAlignment = Alignment.CenterVertically) {
            Row(
                Modifier.clip(RoundedCornerShape(12.dp)).tapRow({ sorting = true }, role = Role.Button, onClickLabel = "Change the order").padding(vertical = 12.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Glyph(R.drawable.ic_arrow_up_down, null, tint = p.secondary, size = 16.dp)
                Spacer(Modifier.width(6.dp))
                Text(sort.label, style = Type.label)
            }
            Spacer(Modifier.weight(1f))
            GlyphButton(if (grid) R.drawable.ic_layout_list else R.drawable.ic_layout_grid, if (grid) "Show as a list" else "Show as a grid", { prefs.setGrid(!grid) })
        }
        Refreshable(library, folders) {
            when (val s = state) {
                Load.Pending -> SkeletonList(rows = 8, artSize = 64.dp)
                is Load.Failed -> Problem(s.message, { library.reload() })
                is Load.Ready -> {
                    val shown = libraryShown(s.value, kind, folder, query, sort)
                    val spaced = if (grid) GRID_GAP else 0.dp
                    LazyVerticalGrid(
                        columns = GridCells.Fixed(if (grid) gridColumns else 1),
                        state = list,
                        contentPadding = PaddingValues(bottom = 24.dp, start = if (grid) 16.dp else 0.dp, end = if (grid) 16.dp else 0.dp),
                        horizontalArrangement = Arrangement.spacedBy(spaced),
                        verticalArrangement = Arrangement.spacedBy(if (grid) 16.dp else 0.dp),
                    ) {
                        if (kind == null && folder == null && query.isBlank()) {
                            if (grid) {
                                item(key = "downloads") { ShortcutTile("Downloads", R.drawable.ic_download, { actions.go(Routes.DOWNLOADS) }) }
                                item(key = "local") { ShortcutTile("Music on this phone", R.drawable.ic_folder, { actions.go(Routes.LOCAL) }) }
                            } else {
                                item(key = "downloads") { Shortcut("Downloads", "Songs on this phone", R.drawable.ic_download) { actions.go(Routes.DOWNLOADS) } }
                                item(key = "local") { Shortcut("Music on this phone", "Your own files", R.drawable.ic_folder) { actions.go(Routes.LOCAL) } }
                            }
                        }
                        items(shown, key = { "${it.kind}:${it.id}" }) { item ->
                            val go = { openLibraryItem(actions, item) }
                            val menu = { actions.menuFor(asItem(item)) }
                            if (grid) {
                                CoverCard(
                                    item.title, item.subtitle, item.artwork, go, size = cell,
                                    round = item.kind == LibraryKind.Artist, onLongClick = menu, titleLines = 2, subtitleLines = 1,
                                )
                            } else {
                                EntityRow(item.title, kindLine(item), item.artwork, go, round = item.kind == LibraryKind.Artist, pinned = item.pinned, onLongClick = menu)
                            }
                        }
                        if (shown.isEmpty()) {
                            item(span = { GridItemSpan(maxLineSpan) }) {
                                Empty(
                                    when {
                                        query.isNotBlank() -> "Nothing matches" to "Nothing in your library has \"${query.trim()}\" in its name."
                                        folder != null -> "This folder is empty" to "Long press something in your library and choose Move to folder."
                                        kind == LibraryKind.Playlist -> "No playlists yet" to "Playlists you make or save show up here."
                                        kind == LibraryKind.Album -> "No albums yet" to "Albums you save show up here."
                                        kind == LibraryKind.Artist -> "No artists yet" to "Artists you follow show up here."
                                        kind == LibraryKind.Podcast -> "No podcasts yet" to "Podcasts you follow show up here."
                                        else -> "Nothing here yet" to "What you save and follow shows up here."
                                    },
                                )
                            }
                        }
                    }
                }
            }
        }
    }

    if (sorting) {
        ChoiceSheet("Sort by", LibrarySort.entries, sort, { it.label }, { prefs.setSort(it) }, { sorting = false })
    }
    if (creating) {
        PageMenu(
            "Create",
            listOf(
                PageAction(R.drawable.ic_list_music, "Playlist", "Build a playlist with songs") { actions.menu = MenuTarget.NewPlaylist },
                PageAction(R.drawable.ic_folder_plus, "Folder", "Keep things in your library together") { naming = true },
            ),
            { creating = false },
        )
    }
    if (naming) {
        TextDialog(
            "New folder", "Create", "", "Name",
            { name, _ ->
                naming = false
                actions.write(Change.Folders, done = "Made $name") { api.createFolder(name) }
            },
            { naming = false },
        )
    }
    if (deleting && open != null) {
        ConfirmDialog(
            "Delete this folder?", "\"${open.name}\" goes. What is in it stays in your library.", "Delete",
            onConfirm = {
                deleting = false
                val id = open.id
                folder = null
                actions.write(Change.Folders, Change.Library, done = "Deleted ${open.name}") { api.deleteFolder(id) }
            },
            onDismiss = { deleting = false },
        )
    }
}

/** One of the app's own lists, above the library: Liked, Downloads, the phone's files. */
@Composable
private fun Shortcut(title: String, subtitle: String, icon: Int, onClick: () -> Unit) {
    val p = LocalPalette.current
    Row(
        Modifier.fillMaxWidth().tapRow(onClick, role = Role.Button, onClickLabel = "Open").padding(horizontal = 16.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(Modifier.size(64.dp).clip(RoundedCornerShape(4.dp)).background(p.accent), contentAlignment = Alignment.Center) {
            Glyph(icon, null, tint = p.onAccent, size = 28.dp)
        }
        Spacer(Modifier.width(12.dp))
        Column {
            Text(title, style = Type.bodyStrong)
            Text(subtitle, style = Type.small, color = p.secondary)
        }
    }
}

private fun openLibraryItem(actions: Actions, item: LibraryItem) = when {
    // Liked Music is a playlist to the core and a page of its own here, which keeps up with a like at once.
    item.kind == LibraryKind.Playlist && bareId(item.id) == "LM" -> actions.go(Routes.LIKED)
    item.kind == LibraryKind.Playlist -> actions.go(Routes.playlist(item.id))
    item.kind == LibraryKind.Album -> actions.go(Routes.album(item.id))
    item.kind == LibraryKind.Artist -> actions.go(Routes.artist(item.id))
    else -> actions.go(Routes.podcast(item.id))
}

private fun asItem(item: LibraryItem): Item = when (item.kind) {
    LibraryKind.Playlist -> Item.PlaylistItem(Playlist(id = item.id, title = item.title, owner = item.subtitle, artwork = item.artwork))
    LibraryKind.Album -> Item.AlbumItem(Album(id = item.id, title = item.title, artwork = item.artwork))
    LibraryKind.Artist -> Item.ArtistItem(Artist(id = item.id, name = item.title, artwork = item.artwork))
    LibraryKind.Podcast -> Item.PodcastItem(Podcast(id = item.id, title = item.title, author = item.subtitle, artwork = item.artwork))
}

/** What a library item is, then what the core says of it: "Playlist • DarkSavci • 50 songs", as Spotify and YouTube Music write it. */
private fun kindLine(item: LibraryItem): String {
    val kind = when (item.kind) {
        LibraryKind.Playlist -> "Playlist"
        LibraryKind.Album -> "Album"
        LibraryKind.Artist -> "Artist"
        LibraryKind.Podcast -> "Podcast"
    }
    val rest = item.subtitle.split(" • ", " · ").map { it.trim() }.filter { it.isNotEmpty() && !it.equals(kind, ignoreCase = true) }
    return (listOf(kind) + rest).joinToString(" • ")
}

/** The narrowest a cover is in the library's grid. */
private val GRID_CELL = 100.dp
private val GRID_GAP = 12.dp
private val GRID_EDGE = 16.dp
