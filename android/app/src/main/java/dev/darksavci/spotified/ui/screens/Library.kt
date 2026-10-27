package dev.darksavci.spotified.ui.screens

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.ui.draw.clip
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import dev.darksavci.spotified.R
import dev.darksavci.spotified.api.Album
import dev.darksavci.spotified.api.Artist
import dev.darksavci.spotified.api.Folder
import dev.darksavci.spotified.api.Item
import dev.darksavci.spotified.api.LibraryItem
import dev.darksavci.spotified.api.LibraryKind
import dev.darksavci.spotified.api.Playlist
import dev.darksavci.spotified.api.Podcast
import dev.darksavci.spotified.ui.Actions
import dev.darksavci.spotified.ui.LocalActions
import dev.darksavci.spotified.ui.Routes
import dev.darksavci.spotified.ui.components.Chip
import dev.darksavci.spotified.ui.components.CoverCard
import dev.darksavci.spotified.ui.components.EntityRow
import dev.darksavci.spotified.ui.components.Glyph
import dev.darksavci.spotified.ui.components.GlyphButton
import dev.darksavci.spotified.ui.components.Load
import dev.darksavci.spotified.ui.components.Loading
import dev.darksavci.spotified.ui.components.Problem
import dev.darksavci.spotified.ui.components.rememberLoader
import dev.darksavci.spotified.ui.theme.LocalPalette
import dev.darksavci.spotified.ui.theme.Type

private enum class Sort(val label: String) { Recent("Recents"), Added("Recently added"), Alphabetical("Alphabetical") }

@Composable
fun LibraryScreen() {
    val actions = LocalActions.current
    val api = actions.graph.api
    val library = rememberLoader("library") { api.library() }
    val state by library.state.collectAsStateWithLifecycle()
    val folders by produceState(emptyList<Folder>()) { value = runCatching { api.folders() }.getOrDefault(emptyList()) }
    var kind by rememberSaveable { mutableStateOf<LibraryKind?>(null) }
    var folder by rememberSaveable { mutableStateOf<String?>(null) }
    var sort by rememberSaveable { mutableStateOf(Sort.Recent) }
    var grid by rememberSaveable { mutableStateOf(false) }

    Column(Modifier.windowInsetsPadding(WindowInsets.statusBars)) {
        Row(Modifier.fillMaxWidth().padding(start = 16.dp, end = 4.dp, top = 8.dp), verticalAlignment = Alignment.CenterVertically) {
            Text(folders.firstOrNull { it.id == folder }?.name ?: "Your Library", style = Type.title, modifier = Modifier.weight(1f))
            GlyphButton(R.drawable.ic_search, "Search", { actions.go(Routes.SEARCH) })
            GlyphButton(R.drawable.ic_plus, "New playlist", { actions.addTo(emptyList()) })
        }
        LazyRow(contentPadding = PaddingValues(horizontal = 16.dp, vertical = 8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            if (folder != null) item { Chip("✕ ${folders.firstOrNull { it.id == folder }?.name.orEmpty()}", true, { folder = null }) }
            items(LibraryKind.entries) { k ->
                Chip(k.name + "s", kind == k, { kind = if (kind == k) null else k })
            }
            items(folders.filter { it.id != folder }) { f -> Chip("📁 ${f.name}", false, { folder = f.id }) }
        }
        Row(Modifier.fillMaxWidth().padding(horizontal = 8.dp), verticalAlignment = Alignment.CenterVertically) {
            Chip(sort.label, false, { sort = Sort.entries[(sort.ordinal + 1) % Sort.entries.size] })
            Spacer(Modifier.weight(1f))
            GlyphButton(if (grid) R.drawable.ic_layout_list else R.drawable.ic_layout_grid, if (grid) "Show as a list" else "Show as a grid", { grid = !grid })
        }
        when (val s = state) {
            Load.Pending -> Loading()
            is Load.Failed -> Problem(s.message, { library.reload() })
            is Load.Ready -> {
                val shown = s.value
                    .filter { kind == null || it.kind == kind }
                    .filter { folder == null || it.folderId == folder }
                    .let { list ->
                        when (sort) {
                            Sort.Recent -> list.sortedByDescending { it.lastPlayedAt ?: it.addedAt ?: "" }
                            Sort.Added -> list.sortedByDescending { it.addedAt ?: "" }
                            Sort.Alphabetical -> list.sortedBy { it.title.lowercase() }
                        }
                    }
                    .sortedByDescending { it.pinned }
                LazyVerticalGrid(
                    columns = if (grid) GridCells.Fixed(3) else GridCells.Fixed(1),
                    contentPadding = PaddingValues(bottom = 24.dp, start = if (grid) 16.dp else 0.dp, end = if (grid) 16.dp else 0.dp),
                    horizontalArrangement = Arrangement.spacedBy(if (grid) 12.dp else 0.dp),
                    verticalArrangement = Arrangement.spacedBy(if (grid) 16.dp else 0.dp),
                ) {
                    if (kind == null && folder == null) {
                        item(span = { GridItemSpan(maxLineSpan) }) {
                            Column {
                                Shortcut("Liked Music", "Playlist • Auto", R.drawable.ic_heart_filled) { actions.go(Routes.LIKED) }
                                Shortcut("Downloads", "Songs on this phone", R.drawable.ic_archive) { actions.go(Routes.DOWNLOADS) }
                                Shortcut("Music on this phone", "Your own files", R.drawable.ic_folder) { actions.go(Routes.LOCAL) }
                            }
                        }
                    }
                    items(shown, key = { "${it.kind}:${it.id}" }) { item ->
                        val open = { openLibraryItem(actions, item) }
                        val menu = { actions.menuFor(asItem(item)) }
                        if (grid) {
                            CoverCard(item.title, item.subtitle, item.artwork, open, size = 104.dp, round = item.kind == LibraryKind.Artist, onLongClick = menu)
                        } else {
                            EntityRow(item.title, item.subtitle, item.artwork, open, round = item.kind == LibraryKind.Artist, pinned = item.pinned, onLongClick = menu)
                        }
                    }
                    if (shown.isEmpty()) item(span = { GridItemSpan(maxLineSpan) }) { Problem("Nothing here yet.", null) }
                }
            }
        }
    }
}

/** One of the app's own lists, above the library: Liked, Downloads, the phone's files. */
@Composable
private fun Shortcut(title: String, subtitle: String, icon: Int, onClick: () -> Unit) {
    val p = LocalPalette.current
    Row(
        Modifier.fillMaxWidth().clickable(onClick = onClick).padding(horizontal = 16.dp, vertical = 8.dp),
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

private fun openLibraryItem(actions: Actions, item: LibraryItem) = when (item.kind) {
    LibraryKind.Playlist -> actions.go(Routes.playlist(item.id))
    LibraryKind.Album -> actions.go(Routes.album(item.id))
    LibraryKind.Artist -> actions.go(Routes.artist(item.id))
    LibraryKind.Podcast -> actions.go(Routes.podcast(item.id))
}

private fun asItem(item: LibraryItem): Item = when (item.kind) {
    LibraryKind.Playlist -> Item.PlaylistItem(Playlist(id = item.id, title = item.title, owner = item.subtitle, artwork = item.artwork))
    LibraryKind.Album -> Item.AlbumItem(Album(id = item.id, title = item.title, artwork = item.artwork))
    LibraryKind.Artist -> Item.ArtistItem(Artist(id = item.id, name = item.title, artwork = item.artwork))
    LibraryKind.Podcast -> Item.PodcastItem(Podcast(id = item.id, title = item.title, author = item.subtitle, artwork = item.artwork))
}
