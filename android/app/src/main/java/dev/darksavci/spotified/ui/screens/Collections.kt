package dev.darksavci.spotified.ui.screens

import dev.darksavci.spotified.ui.components.FadingTopBar
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.ui.draw.shadow
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import dev.darksavci.spotified.R
import dev.darksavci.spotified.api.Artwork
import dev.darksavci.spotified.api.Item
import dev.darksavci.spotified.api.LibraryKind
import dev.darksavci.spotified.api.Playlist
import dev.darksavci.spotified.api.Track
import dev.darksavci.spotified.ui.Actions
import dev.darksavci.spotified.ui.LocalActions
import dev.darksavci.spotified.ui.Routes
import dev.darksavci.spotified.ui.components.Art
import dev.darksavci.spotified.ui.components.GlyphButton
import dev.darksavci.spotified.ui.components.Loaded
import dev.darksavci.spotified.ui.components.Pill
import dev.darksavci.spotified.ui.components.PlayButton
import dev.darksavci.spotified.ui.components.SectionTitle
import dev.darksavci.spotified.ui.components.ShelfRow
import dev.darksavci.spotified.ui.components.TrackRow
import dev.darksavci.spotified.ui.components.artColor
import dev.darksavci.spotified.ui.components.darkened
import dev.darksavci.spotified.ui.components.rememberLoader
import dev.darksavci.spotified.ui.player.clock
import dev.darksavci.spotified.ui.theme.LocalPalette
import dev.darksavci.spotified.ui.theme.Type

/**
 * The top of an album, playlist or podcast: a gradient from the art's colour,
 * the art, the title and byline, and a row of actions with the play button.
 */
fun LazyListScope.collectionHeader(
    title: String,
    byline: String,
    detail: String,
    artwork: List<Artwork>,
    tracks: List<Track>,
    origin: String,
    actions: Actions,
    bylineArtwork: List<Artwork> = emptyList(),
    extra: @Composable () -> Unit = {},
) {
    item {
        val p = LocalPalette.current
        val tint = artColor(artwork)
        val projection by actions.graph.playback.projection.collectAsStateWithLifecycle()
        val playingHere = projection.state.queue.origin == origin && projection.target.playing
        val downloads by actions.graph.downloads.done.collectAsStateWithLifecycle()
        val allDownloaded = tracks.isNotEmpty() && tracks.all { it.id in downloads }
        Column(Modifier.background(Brush.verticalGradient(listOf(tint.darkened(0.2f), p.window)))) {
            // Room for the bar that floats over the page.
            Spacer(Modifier.windowInsetsPadding(WindowInsets.statusBars).height(56.dp))
            Art(artwork, 240.dp, Modifier.align(Alignment.CenterHorizontally).shadow(24.dp, RoundedCornerShape(4.dp)))
            Spacer(Modifier.height(20.dp))
            Text(title, style = Type.title, modifier = Modifier.padding(horizontal = 16.dp), maxLines = 3, overflow = TextOverflow.Ellipsis)
            if (byline.isNotEmpty()) {
                Row(Modifier.padding(horizontal = 16.dp, vertical = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                    if (bylineArtwork.isNotEmpty()) {
                        Art(bylineArtwork, 24.dp, shape = CircleShape)
                        Spacer(Modifier.width(8.dp))
                    }
                    Text(byline, style = Type.bodyStrong)
                }
            }
            if (detail.isNotEmpty()) Text(detail, style = Type.small, color = p.secondary, modifier = Modifier.padding(horizontal = 16.dp))
            Row(Modifier.fillMaxWidth().padding(horizontal = 8.dp, vertical = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                GlyphButton(
                    if (allDownloaded) R.drawable.ic_circle_check else R.drawable.ic_archive,
                    if (allDownloaded) "Downloaded" else "Download",
                    { if (allDownloaded) actions.graph.downloads.remove(tracks) else actions.graph.downloads.add(tracks) },
                    tint = if (allDownloaded) p.accent else p.secondary,
                )
                GlyphButton(R.drawable.ic_list_end, "Add all to the queue", { actions.playback.addToQueue(tracks) }, tint = p.secondary)
                extra()
                Spacer(Modifier.weight(1f))
                GlyphButton(R.drawable.ic_shuffle, "Shuffle", { actions.playback.shuffle(tracks, origin) }, tint = p.secondary)
                Spacer(Modifier.padding(4.dp))
                PlayButton(
                    onClick = { if (playingHere) actions.playback.toggle() else actions.play(tracks, 0, origin) },
                    playing = playingHere,
                )
            }
        }
    }
}

fun durationText(tracks: List<Track>): String {
    val total = tracks.sumOf { it.durationMs }
    if (total <= 0) return "${tracks.size} songs"
    val minutes = total / 60_000
    return "${tracks.size} songs • " + if (minutes >= 60) "${minutes / 60} hr ${minutes % 60} min" else "$minutes min"
}

@Composable
fun AlbumScreen(id: String) {
    val actions = LocalActions.current
    val loader = rememberLoader("album:$id") { actions.graph.api.album(id) }
    Loaded(loader) { album ->
        val tracks = album.tracks.map { if (it.artwork.isEmpty()) it.copy(artwork = album.artwork) else it }
        val projection by actions.graph.playback.projection.collectAsStateWithLifecycle()
        val downloads by actions.graph.downloads.done.collectAsStateWithLifecycle()
        val list = rememberLazyListState()
        Box {
        LazyColumn(state = list, contentPadding = PaddingValues(bottom = 24.dp)) {
            collectionHeader(
                album.title, album.artists.joinToString { it.name },
                listOf(album.kind, album.year, durationText(tracks)).filter { it.isNotEmpty() }.joinToString(" • "),
                album.artwork, tracks, album.title, actions,
                bylineArtwork = album.artistArtwork,
                extra = {
                    GlyphButton(R.drawable.ic_pin, "Pin in your library", {
                        actions.launch { actions.graph.api.organise(LibraryKind.Album, album.id, pinned = true); actions.say("Pinned") }
                    }, tint = LocalPalette.current.secondary)
                    GlyphButton(R.drawable.ic_share_2, "Share", { actions.share(album.title, Actions.albumLink(album.id)) }, tint = LocalPalette.current.secondary)
                },
            )
            itemsIndexed(tracks) { i, t ->
                TrackRow(
                    t, { actions.play(tracks, i, album.title) }, { actions.menuFor(t) },
                    playing = projection.state.current?.id == t.id, number = i + 1, showArt = false,
                    downloaded = t.id in downloads,
                )
            }
            if (album.description.isNotEmpty()) {
                item { Text(album.description, style = Type.small, color = LocalPalette.current.secondary, modifier = Modifier.padding(16.dp)) }
            }
            items(album.shelves.filter { it.items.isNotEmpty() }) { shelf ->
                SectionTitle(shelf.title)
                ShelfRow(shelf.items, actions::open, actions::menuFor)
            }
        }
        FadingTopBar(album.title, artColor(album.artwork), list) { actions.nav.popBackStack() }
        }
    }
}

@Composable
fun PlaylistScreen(id: String) {
    val actions = LocalActions.current
    val loader = rememberLoader("playlist:$id") { actions.graph.api.completePlaylist(id) }
    Loaded(loader) { playlist -> TrackList(playlist, playlist.title, playlist.owner, playlist.artwork, editable = playlist.editable) }
}

@Composable
private fun TrackList(playlist: Playlist, origin: String, byline: String, artwork: List<Artwork>, editable: Boolean) {
    val actions = LocalActions.current
    val projection by actions.graph.playback.projection.collectAsStateWithLifecycle()
    val downloads by actions.graph.downloads.done.collectAsStateWithLifecycle()
    val tracks = playlist.tracks
    val list = rememberLazyListState()
    Box {
    LazyColumn(state = list, contentPadding = PaddingValues(bottom = 24.dp)) {
        collectionHeader(
            playlist.title, byline, durationText(tracks), artwork.ifEmpty { tracks.firstOrNull()?.artwork.orEmpty() },
            tracks, origin, actions,
            extra = {
                if (playlist.id.isNotEmpty()) {
                    GlyphButton(R.drawable.ic_share_2, "Share", { actions.share(playlist.title, Actions.playlistLink(playlist.id)) }, tint = LocalPalette.current.secondary)
                }
            },
        )
        if (editable) {
            item {
                LazyRow(contentPadding = PaddingValues(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    item { Pill("Add", { actions.go(Routes.SEARCH) }, icon = R.drawable.ic_plus) }
                    item { Pill("Download and keep in step", { actions.launch { actions.graph.downloads.addPlaylist(playlist.id); actions.say("Downloading ${playlist.title}") } }, icon = R.drawable.ic_archive) }
                    item {
                        Pill("Delete", {
                            actions.launch {
                                actions.graph.api.deletePlaylist(playlist.id)
                                actions.say("Deleted ${playlist.title}")
                            }
                            actions.nav.popBackStack()
                        }, icon = R.drawable.ic_trash_2)
                    }
                }
            }
        }
        itemsIndexed(tracks) { i, t ->
            TrackRow(
                t, { actions.play(tracks, i, origin) }, { actions.menuFor(t, playlist = playlist) },
                playing = projection.state.current?.id == t.id, downloaded = t.id in downloads,
            )
        }
    }
    FadingTopBar(playlist.title, artColor(artwork.ifEmpty { tracks.firstOrNull()?.artwork.orEmpty() }), list) { actions.nav.popBackStack() }
    }
}

@Composable
fun LikedScreen(shuffle: Boolean = false) {
    val actions = LocalActions.current
    val loader = rememberLoader("liked") { actions.graph.api.liked() }
    Loaded(loader) { liked ->
        LaunchedEffect(shuffle) { if (shuffle) actions.playback.shuffle(liked.tracks, "Liked Music") }
        TrackList(liked.copy(title = "Liked Music"), "Liked Music", "Auto playlist", emptyList(), editable = false)
    }
}

@Composable
fun PodcastScreen(id: String) {
    val actions = LocalActions.current
    val loader = rememberLoader("podcast:$id") { actions.graph.api.podcast(id) }
    Loaded(loader) { podcast ->
        val list = rememberLazyListState()
        Box {
        LazyColumn(state = list, contentPadding = PaddingValues(bottom = 24.dp)) {
            collectionHeader(podcast.title, podcast.author, "${podcast.episodes.size} episodes", podcast.artwork, podcast.episodes, podcast.title, actions)
            if (podcast.description.isNotEmpty()) {
                item { Text(podcast.description, style = Type.small, color = LocalPalette.current.secondary, modifier = Modifier.padding(16.dp), maxLines = 6, overflow = TextOverflow.Ellipsis) }
            }
            itemsIndexed(podcast.episodes) { i, e ->
                TrackRow(e, { actions.play(podcast.episodes, i, podcast.title) }, { actions.menuFor(e) }, subtitle = clock(e.durationMs).takeIf { e.durationMs > 0 } ?: podcast.author)
            }
        }
        FadingTopBar(podcast.title, artColor(podcast.artwork), list) { actions.nav.popBackStack() }
        }
    }
}

@Composable
fun HistoryScreen() {
    val actions = LocalActions.current
    val loader = rememberLoader("history") { actions.graph.api.history() }
    Loaded(loader) { tracks ->
        LazyColumn(Modifier.windowInsetsPadding(WindowInsets.statusBars), contentPadding = PaddingValues(bottom = 24.dp)) {
            item {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    GlyphButton(R.drawable.ic_chevron_left, "Back", { actions.nav.popBackStack() })
                    Text("Recently played", style = Type.title)
                }
            }
            itemsIndexed(tracks) { i, t -> TrackRow(t, { actions.play(tracks, i, "Recently played") }, { actions.menuFor(t) }) }
        }
    }
}

/** Any other surface: a mood's page, the whole of a shelf, Discover, the charts. */
@Composable
fun BrowseScreen(id: String, params: String, title: String) {
    val actions = LocalActions.current
    val loader = rememberLoader("browse:$id:$params") { actions.graph.api.browse(id, params) }
    Loaded(loader) { page ->
        LazyColumn(Modifier.windowInsetsPadding(WindowInsets.statusBars), contentPadding = PaddingValues(bottom = 24.dp)) {
            item {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    GlyphButton(R.drawable.ic_chevron_left, "Back", { actions.nav.popBackStack() })
                    Text(page.title.ifEmpty { title }, style = Type.title, maxLines = 1, overflow = TextOverflow.Ellipsis)
                }
            }
            if (page.moods.isNotEmpty()) {
                item {
                    LazyRow(contentPadding = PaddingValues(16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        items(page.moods) { m -> Pill(m.title, { actions.go(Routes.browse(m.id, m.params, m.title)) }) }
                    }
                }
            }
            items(page.shelves.filter { it.items.isNotEmpty() }) { shelf ->
                if (shelf.title.isNotEmpty()) SectionTitle(shelf.title)
                val songs = shelf.items.filterIsInstance<Item.Song>().map { it.track }
                if (songs.size == shelf.items.size) {
                    Column { songs.forEachIndexed { i, t -> TrackRow(t, { actions.play(songs, i, shelf.title.ifEmpty { title }) }, { actions.menuFor(t) }) } }
                } else {
                    ShelfRow(shelf.items, actions::open, actions::menuFor)
                }
            }
        }
    }
}
