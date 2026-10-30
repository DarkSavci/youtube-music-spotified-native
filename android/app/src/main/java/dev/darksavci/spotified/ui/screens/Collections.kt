package dev.darksavci.spotified.ui.screens

import androidx.compose.ui.draw.clip
import dev.darksavci.spotified.ui.components.tapRow
import dev.darksavci.spotified.data.DownloadStatus
import dev.darksavci.spotified.data.DownloadPlan
import dev.darksavci.spotified.api.ArtistRef
import androidx.compose.foundation.layout.size
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
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
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.listSaver
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import dev.darksavci.spotified.R
import dev.darksavci.spotified.api.Artwork
import dev.darksavci.spotified.api.Item
import dev.darksavci.spotified.api.LibraryItem
import dev.darksavci.spotified.api.LibraryKind
import dev.darksavci.spotified.api.Playlist
import dev.darksavci.spotified.api.Track
import dev.darksavci.spotified.ui.Actions
import dev.darksavci.spotified.ui.MenuTarget
import dev.darksavci.spotified.ui.Shared
import dev.darksavci.spotified.ui.Change
import dev.darksavci.spotified.ui.LocalActions
import dev.darksavci.spotified.ui.Routes
import dev.darksavci.spotified.ui.components.Art
import dev.darksavci.spotified.ui.components.ChoiceSheet
import dev.darksavci.spotified.ui.components.ConfirmDialog
import dev.darksavci.spotified.ui.components.Empty
import dev.darksavci.spotified.ui.components.ExpandableText
import dev.darksavci.spotified.ui.components.FadingTopBar
import dev.darksavci.spotified.ui.components.GlyphButton
import dev.darksavci.spotified.ui.components.Load
import dev.darksavci.spotified.ui.components.LocalPlayingId
import dev.darksavci.spotified.ui.components.Loaded
import dev.darksavci.spotified.ui.components.Loader
import dev.darksavci.spotified.ui.components.PageAction
import dev.darksavci.spotified.ui.components.PageMenu
import dev.darksavci.spotified.ui.components.Pill
import dev.darksavci.spotified.ui.components.PlayButton
import dev.darksavci.spotified.ui.components.Refreshable
import dev.darksavci.spotified.ui.components.SectionTitle
import dev.darksavci.spotified.ui.components.SelectionAction
import dev.darksavci.spotified.ui.components.SelectionBar
import dev.darksavci.spotified.ui.components.ShelfRow
import dev.darksavci.spotified.ui.components.SkeletonList
import dev.darksavci.spotified.ui.components.SkeletonShelves
import dev.darksavci.spotified.ui.components.SubPage
import dev.darksavci.spotified.ui.components.TextDialog
import dev.darksavci.spotified.ui.components.TitleBar
import dev.darksavci.spotified.ui.components.TrackRow
import dev.darksavci.spotified.ui.components.artColor
import dev.darksavci.spotified.ui.components.darkened
import dev.darksavci.spotified.ui.components.predictiveBack
import dev.darksavci.spotified.ui.components.rememberLoader
import dev.darksavci.spotified.ui.player.clock
import dev.darksavci.spotified.ui.theme.LocalPalette
import dev.darksavci.spotified.ui.theme.Type

/** Whether the queue is [tracks], and whether it is playing. */
@Composable
internal fun rememberPlayingHere(tracks: List<Track>, origin: String, actions: Actions): Pair<Boolean, Boolean> {
    val projection by actions.graph.playback.projection.collectAsStateWithLifecycle()
    val queue = projection.state.queue
    val here = isQueueOf(queue.items, queue.index, queue.origin, tracks, origin)
    return here to (here && projection.target.playing)
}

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
    bylineArtists: List<ArtistRef> = emptyList(),
    extra: @Composable () -> Unit = {},
) {
    item {
        val p = LocalPalette.current
        val tint = artColor(artwork)
        val (here, playing) = rememberPlayingHere(tracks, origin, actions)
        val downloads by actions.graph.downloads.done.collectAsStateWithLifecycle()
        val allDownloaded = tracks.isNotEmpty() && tracks.all { it.id in downloads }
        val wanted by actions.graph.downloads.tracks.collectAsStateWithLifecycle()
        val statuses by actions.graph.downloads.statuses.collectAsStateWithLifecycle()
        val coming = remember(tracks, wanted, downloads) { downloadsComing(tracks, wanted.mapTo(HashSet()) { it.id }, downloads) }
        var removing by remember { mutableStateOf(false) }
        Column(Modifier.background(Brush.verticalGradient(listOf(tint.darkened(0.2f), p.window)))) {
            // Room for the bar that floats over the page.
            Spacer(Modifier.windowInsetsPadding(WindowInsets.statusBars).height(56.dp))
            Art(artwork, 240.dp, Modifier.align(Alignment.CenterHorizontally).shadow(24.dp, RoundedCornerShape(4.dp)), description = "Cover of $title")
            Spacer(Modifier.height(20.dp))
            Text(title, style = Type.title, modifier = Modifier.padding(horizontal = 16.dp), maxLines = 3, overflow = TextOverflow.Ellipsis)
            if (byline.isNotEmpty()) {
                // An album by several: each is as much its artist as the first.
                val linked = bylineArtists.filter { it.id.isNotEmpty() }
                val toArtist = Modifier.clip(RoundedCornerShape(16.dp)).tapRow(
                    {
                        if (linked.size > 1) actions.menu = MenuTarget.ArtistsOf(linked)
                        else actions.go(Routes.artist(linked.first().id))
                    },
                    role = Role.Button,
                    onClickLabel = if (linked.size > 1) "Choose an artist" else "Open the artist",
                )
                Row(
                    Modifier.padding(horizontal = 8.dp, vertical = 4.dp).then(if (linked.isNotEmpty()) toArtist else Modifier).padding(horizontal = 8.dp, vertical = 4.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    if (bylineArtwork.isNotEmpty()) {
                        Art(bylineArtwork, 24.dp, shape = CircleShape)
                        Spacer(Modifier.width(8.dp))
                    }
                    Text(byline, style = Type.bodyStrong)
                }
            }
            if (detail.isNotEmpty()) Text(detail, style = Type.small, color = p.secondary, modifier = Modifier.padding(horizontal = 16.dp))
            Row(Modifier.fillMaxWidth().padding(horizontal = 8.dp, vertical = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                if (coming.isNotEmpty()) {
                    // A ring of how far the batch has got, with the page that says more behind it.
                    val fraction = DownloadPlan.overall(tracks.map { it.id }, statuses + downloads.associateWith { DownloadStatus.Done })
                    Box(
                        Modifier.size(48.dp).clip(CircleShape).tapRow(
                            { actions.go(Routes.DOWNLOADS) }, role = Role.Button,
                            onClickLabel = "See the downloads",
                        ).semantics { contentDescription = "Downloading ${songs(coming.size)}, ${(fraction * 100).toInt()}%" },
                        contentAlignment = Alignment.Center,
                    ) {
                        CircularProgressIndicator({ fraction }, Modifier.size(24.dp), color = p.accent, trackColor = p.surface, strokeWidth = 2.5.dp)
                    }
                } else {
                    GlyphButton(
                        if (allDownloaded) R.drawable.ic_downloaded else R.drawable.ic_download,
                        if (allDownloaded) "Remove the downloads" else "Download",
                        {
                            if (allDownloaded) removing = true
                            else {
                                actions.graph.downloads.add(tracks)
                                actions.say("Downloading ${songs(tracks.size)}")
                            }
                        },
                        tint = if (allDownloaded) p.accent else p.secondary,
                    )
                }
                extra()
                Spacer(Modifier.weight(1f))
                GlyphButton(R.drawable.ic_shuffle, "Shuffle", { actions.playback.shuffle(tracks, origin) }, tint = p.secondary)
                Spacer(Modifier.padding(4.dp))
                PlayButton(
                    onClick = { if (here) actions.playback.toggle() else actions.play(tracks, 0, origin) },
                    playing = playing,
                    // The row's edge is 8dp; the button lines up with the 16dp of the words above.
                    modifier = Modifier.padding(end = 8.dp),
                )
            }
        }
        if (removing) {
            ConfirmDialog(
                "Remove the downloads?", "${songs(tracks.size)} will no longer be on this phone. You can download them again.", "Remove",
                onConfirm = { removing = false; actions.graph.downloads.remove(tracks); actions.say("Downloads removed") },
                onDismiss = { removing = false },
            )
        }
    }
}

fun durationText(tracks: List<Track>): String {
    val total = tracks.sumOf { it.durationMs }
    if (total <= 0) return songs(tracks.size)
    val minutes = total / 60_000
    return "${songs(tracks.size)} • " + if (minutes >= 60) "${minutes / 60} hr ${minutes % 60} min" else "$minutes min"
}

// ---------- what a list's songs can have done to them ----------

/** Likes [tracks], asking first if there are many, each write being its own request; Undo gives back what this changed. */
internal fun likeAll(actions: Actions, tracks: List<Track>) {
    val liked = actions.graph.liked
    val fresh = tracks.filter { it.id.isNotEmpty() && !liked.isLiked(it.id) }.distinctBy { it.id }
    if (fresh.isEmpty()) return actions.say("Already in your Liked Music")
    val run = {
        fresh.forEach { liked.set(it.id, true) }
        actions.sayWithUndo("Liked ${songs(fresh.size)}") { fresh.forEach { liked.set(it.id, false) } }
    }
    if (fresh.size > LIKE_ASKS_ABOVE) actions.confirm("Like ${songs(fresh.size)}?", "They will all go in your Liked Music.", "Like all", danger = false, run = run)
    else run()
}

/** Takes [tracks] out of [playlist] at once, with Undo, which adds them back at its end. */
fun removeFromPlaylist(actions: Actions, playlist: Playlist, tracks: List<Track>) {
    val placed = tracks.filter { it.playlistItemId.isNotEmpty() }
    if (placed.isEmpty()) return
    val api = actions.graph.api
    actions.write(Change.Playlist(playlist.id), Change.Library) {
        api.removeFromPlaylist(playlist.id, placed.map { it.id to it.playlistItemId })
    }
    val what = if (placed.size == 1) "Removed from ${playlist.title}" else "Removed ${songs(placed.size)} from ${playlist.title}"
    actions.sayWithUndo(what) {
        actions.write(Change.Playlist(playlist.id), Change.Library) { api.addToPlaylist(playlist.id, placed.map { it.id }) }
    }
}

/** Saves to the library or takes out, which can be undone either way. */
internal fun setSaved(actions: Actions, kind: LibraryKind, id: String, title: String, saved: Boolean) {
    val api = actions.graph.api
    actions.write(Change.Library) { api.setSaved(kind, id, saved) }
    actions.sayWithUndo(if (saved) "Saved $title to your library" else "Took $title out of your library") {
        actions.write(Change.Library) { api.setSaved(kind, id, !saved) }
    }
}

/** The library, to know whether a page's album or playlist is in it. */
@Composable
private fun rememberLibraryEntry(kind: LibraryKind, id: String, actions: Actions): LibraryItem? {
    val library = rememberLoader("library:has", follows = setOf(Change.Library)) { actions.graph.api.library() }
    val state by library.state.collectAsStateWithLifecycle()
    return (state as? Load.Ready)?.value?.let { libraryEntry(it, kind, id) }
}

/** The save, a tap to save and a tap to take out. */
@Composable
private fun SaveButton(saved: Boolean, onClick: () -> Unit) {
    val p = LocalPalette.current
    GlyphButton(
        if (saved) R.drawable.ic_check else R.drawable.ic_circle_plus,
        if (saved) "Remove from your library" else "Save to your library",
        onClick, tint = if (saved) p.accent else p.secondary,
    )
}

// ---------- choosing songs from a list ----------

/** The songs chosen from a list. Empty and off, it is an ordinary list. */
internal class TrackSelection(active: Boolean, chosen: Set<String>) {
    var active by mutableStateOf(active)
        private set
    var chosen by mutableStateOf(chosen)
        private set

    fun start(key: String) {
        active = true
        chosen = setOf(key)
    }

    fun toggle(key: String) {
        chosen = if (key in chosen) chosen - key else chosen + key
    }

    fun setAll(keys: List<String>) {
        chosen = if (chosen.size == keys.size) emptySet() else keys.toSet()
    }

    fun stop() {
        active = false
        chosen = emptySet()
    }

    companion object {
        val Saver = listSaver<TrackSelection, String>(
            save = { listOf(it.active.toString()) + it.chosen },
            restore = { TrackSelection(it.first().toBoolean(), it.drop(1).toSet()) },
        )
    }
}

@Composable
internal fun rememberTrackSelection(): TrackSelection {
    val selection = rememberSaveable(saver = TrackSelection.Saver) { TrackSelection(false, emptySet()) }
    predictiveBack(selection.active) { selection.stop() }
    return selection
}

/** The bar of a list being chosen from, doing to the chosen songs what a song's menu does to one. */
@Composable
internal fun BoxScope.TrackSelectionBar(
    selection: TrackSelection,
    tracks: List<Track>,
    keys: List<String>,
    actions: Actions,
    playlist: Playlist? = null,
) {
    if (!selection.active) return
    val chosen = chosenSongs(tracks, keys, selection.chosen)
    val done = { selection.stop() }
    SelectionBar(
        selection.chosen.size, keys.size, { selection.setAll(keys) }, done,
        buildList {
            add(SelectionAction(R.drawable.ic_list_end, "Queue") {
                val playable = chosen.filter { it.playable }
                actions.playback.addToQueue(playable)
                actions.say("Added ${songs(playable.size)} to the queue")
                done()
            })
            add(SelectionAction(R.drawable.ic_heart, "Like") { likeAll(actions, chosen); done() })
            add(SelectionAction(R.drawable.ic_circle_plus, "Playlist") { actions.addTo(chosen); done() })
            add(SelectionAction(R.drawable.ic_download, "Download") {
                actions.graph.downloads.add(chosen)
                actions.say("Downloading ${songs(chosen.size)}")
                done()
            })
            if (playlist != null && playlist.editable) {
                add(SelectionAction(R.drawable.ic_trash_2, "Remove") { removeFromPlaylist(actions, playlist, chosen); done() })
            }
        },
    )
}

/** A row of a list that can be chosen from: a long press starts choosing, a tap then chooses. */
@Composable
internal fun SelectableRow(
    track: Track,
    key: String,
    selection: TrackSelection,
    onPlay: () -> Unit,
    onMenu: () -> Unit,
    number: Int? = null,
    showArt: Boolean = true,
    downloaded: Boolean = false,
    subtitle: String? = null,
    modifier: Modifier = Modifier,
) {
    TrackRow(
        track,
        { if (selection.active) selection.toggle(key) else onPlay() },
        onMenu,
        modifier,
        number = number,
        showArt = showArt,
        downloaded = downloaded,
        subtitle = subtitle,
        onLongPress = { if (!selection.active) selection.start(key) else selection.toggle(key) },
        selected = if (selection.active) key in selection.chosen else null,
    )
}

// ---------- the pages ----------

@Composable
fun AlbumScreen(id: String) {
    val actions = LocalActions.current
    val loader = rememberLoader("album:$id", follows = setOf(Change.Everything)) { actions.graph.api.album(id) }
    SubPage(loader) { album ->
        val p = LocalPalette.current
        val tracks = album.tracks.map { if (it.artwork.isEmpty()) it.copy(artwork = album.artwork) else it }
        val keys = remember(tracks) { rowKeys(tracks) }
        val downloads by actions.graph.downloads.done.collectAsStateWithLifecycle()
        val list = rememberLazyListState()
        val selection = rememberTrackSelection()
        val entry = rememberLibraryEntry(LibraryKind.Album, album.id, actions)
        var more by remember { mutableStateOf(false) }
        Refreshable(loader) {
            Box(Modifier.fillMaxSize()) {
                LazyColumn(state = list, contentPadding = PaddingValues(bottom = if (selection.active) 140.dp else 24.dp)) {
                    collectionHeader(
                        album.title, album.artists.joinToString { it.name },
                        listOf(album.kind, album.year, durationText(tracks)).filter { it.isNotEmpty() }.joinToString(" • "),
                        album.artwork, tracks, album.title, actions,
                        bylineArtwork = album.artistArtwork,
                        bylineArtists = album.artists,
                        extra = {
                            SaveButton(entry != null) { setSaved(actions, LibraryKind.Album, album.id, album.title, entry == null) }
                            GlyphButton(R.drawable.ic_ellipsis, "More for this album", { more = true }, tint = p.secondary)
                        },
                    )
                    itemsIndexed(tracks, key = { i, _ -> keys[i] }) { i, t ->
                        SelectableRow(
                            t, keys[i], selection, { actions.play(tracks, i, album.title) }, { actions.menuFor(t) },
                            number = i + 1, showArt = false, downloaded = t.id in downloads,
                        )
                    }
                    if (album.description.isNotEmpty()) {
                        item { ExpandableText(album.description, 4, Modifier.padding(16.dp), style = Type.small, color = p.secondary) }
                    }
                    items(album.shelves.filter { it.items.isNotEmpty() }) { shelf ->
                        SectionTitle(shelf.title)
                        ShelfRow(shelf.items, actions::open, actions::menuFor)
                    }
                }
                FadingTopBar(album.title, artColor(album.artwork), list) { actions.nav.popBackStack() }
                TrackSelectionBar(selection, tracks, keys, actions)
            }
        }
        if (more) {
            PageMenu(
                album.title,
                buildList {
                    add(PageAction(R.drawable.ic_heart, "Like all songs") { likeAll(actions, tracks) })
                    add(PageAction(R.drawable.ic_list_end, "Add all to the queue") { actions.playback.addToQueue(tracks); actions.say("Added to the queue") })
                    add(PageAction(R.drawable.ic_circle_plus, "Add all to a playlist") { actions.addTo(tracks) })
                    add(PageAction(R.drawable.ic_check, "Select songs") { keys.firstOrNull()?.let(selection::start) })
                    if (entry != null) add(pinAction(actions, entry))
                    add(PageAction(R.drawable.ic_share_2, "Share") { actions.menu = MenuTarget.Share(Shared.AlbumOf(album)) })
                },
                { more = false },
            )
        }
    }
}

/** Pins the library's line for a page, or takes the pin off. */
private fun pinAction(actions: Actions, entry: LibraryItem) = if (entry.pinned) {
    PageAction(R.drawable.ic_pin_off, "Unpin from your library") {
        actions.write(Change.Library, done = "Unpinned") { actions.graph.api.organise(entry.kind, entry.id, pinned = false) }
    }
} else {
    PageAction(R.drawable.ic_pin, "Pin in your library") {
        actions.write(Change.Library, done = "Pinned") { actions.graph.api.organise(entry.kind, entry.id, pinned = true) }
    }
}

@Composable
fun PlaylistScreen(id: String) {
    val actions = LocalActions.current
    val loader = rememberLoader("playlist:$id", follows = setOf(Change.Playlist(id))) { actions.graph.api.completePlaylist(id) }
    SubPage(loader) { playlist ->
        TrackList(playlist, playlist.title, playlist.owner, playlist.artwork, editable = playlist.editable, loader = loader, tracks = playlist.tracks)
    }
}

@Composable
private fun TrackList(
    playlist: Playlist,
    origin: String,
    byline: String,
    artwork: List<Artwork>,
    editable: Boolean,
    loader: Loader<Playlist>,
    tracks: List<Track>,
    liked: Boolean = false,
) {
    val actions = LocalActions.current
    val p = LocalPalette.current
    val downloads by actions.graph.downloads.done.collectAsStateWithLifecycle()
    var sort by rememberSaveable { mutableStateOf(SongSort.Custom) }
    val shown = remember(tracks, sort) { sortedSongs(tracks, sort) }
    val keys = remember(shown) { rowKeys(shown) }
    val list = rememberLazyListState()
    val selection = rememberTrackSelection()
    val entry = if (liked) null else rememberLibraryEntry(LibraryKind.Playlist, playlist.id, actions)
    var more by remember { mutableStateOf(false) }
    var sorting by remember { mutableStateOf(false) }
    var editing by remember { mutableStateOf(false) }
    val cover = artwork.ifEmpty { shown.firstOrNull()?.artwork.orEmpty() }
    Refreshable(loader) {
        Box(Modifier.fillMaxSize()) {
            LazyColumn(state = list, contentPadding = PaddingValues(bottom = if (selection.active) 140.dp else 24.dp)) {
                collectionHeader(
                    playlist.title, byline, durationText(tracks), cover, shown, origin, actions,
                    extra = {
                        if (!editable && !liked && playlist.id.isNotEmpty()) {
                            SaveButton(entry != null) { setSaved(actions, LibraryKind.Playlist, playlist.id, playlist.title, entry == null) }
                        }
                        GlyphButton(R.drawable.ic_ellipsis, "More for this list", { more = true }, tint = p.secondary)
                    },
                )
                if (playlist.description.isNotEmpty()) {
                    item { ExpandableText(playlist.description, 3, Modifier.padding(horizontal = 16.dp, vertical = 4.dp), style = Type.small, color = p.secondary) }
                }
                item {
                    LazyRow(contentPadding = PaddingValues(horizontal = 16.dp, vertical = 4.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        if (editable) {
                            item { Pill("Add", { actions.go(Routes.playlistAdd(playlist.id, playlist.title)) }, icon = R.drawable.ic_plus, description = "Add songs to this playlist") }
                        }
                        item { Pill(if (sort == SongSort.Custom) "Sort" else sort.label, { sorting = true }, icon = R.drawable.ic_arrow_up_down, description = "Sort the songs") }
                        if (editable) {
                            item { Pill("Edit", { editing = true }, icon = R.drawable.ic_pencil, description = "Rename or describe this playlist") }
                            item { Pill("Download and keep in step", { actions.launch {
                                actions.graph.downloads.addPlaylist(playlist.id)
                                actions.say(if (actions.graph.downloads.waitingForWifi.value) "${playlist.title} will download on Wi-Fi" else "Downloading ${playlist.title}")
                            } }, icon = R.drawable.ic_download) }
                            item {
                                Pill("Delete", {
                                    actions.confirm("Delete this playlist?", "\"${playlist.title}\" will be gone from your account. This can't be undone.", "Delete") {
                                        actions.write(Change.Library, Change.Playlist(playlist.id), done = "Deleted ${playlist.title}") { actions.graph.api.deletePlaylist(playlist.id) }
                                        actions.nav.popBackStack()
                                    }
                                }, icon = R.drawable.ic_trash_2, description = "Delete this playlist")
                            }
                        }
                    }
                }
                itemsIndexed(shown, key = { i, _ -> keys[i] }) { i, t ->
                    SelectableRow(
                        t, keys[i], selection, { actions.play(shown, i, origin) }, { actions.menuFor(t, playlist = playlist) },
                        downloaded = t.id in downloads, modifier = Modifier.animateItem(),
                    )
                }
                if (shown.isEmpty()) {
                    item {
                        Empty(
                            if (liked) "No liked songs yet" to "Songs you like show up here."
                            else if (editable) "No songs yet" to "Add some with the button above."
                            else "No songs here" to "This list came back empty.",
                        )
                    }
                }
            }
            FadingTopBar(playlist.title, artColor(cover), list) { actions.nav.popBackStack() }
            TrackSelectionBar(selection, shown, keys, actions, playlist.takeIf { editable })
        }
    }
    if (more) {
        PageMenu(
            playlist.title,
            buildList {
                if (!liked) add(PageAction(R.drawable.ic_heart, "Like all songs") { likeAll(actions, shown) })
                add(PageAction(R.drawable.ic_list_end, "Add all to the queue") { actions.playback.addToQueue(shown); actions.say("Added to the queue") })
                add(PageAction(R.drawable.ic_circle_plus, "Add all to a playlist") { actions.addTo(shown) })
                add(PageAction(R.drawable.ic_check, "Select songs") { keys.firstOrNull()?.let(selection::start) })
                if (entry != null) add(pinAction(actions, entry))
                if (playlist.id.isNotEmpty() && !liked) add(PageAction(R.drawable.ic_share_2, "Share") { actions.menu = MenuTarget.Share(Shared.PlaylistOf(playlist.id, playlist.title)) })
            },
            { more = false },
        )
    }
    if (sorting) {
        ChoiceSheet("Sort by", SongSort.entries, sort, { it.label }, { sort = it }, { sorting = false })
    }
    if (editing) {
        TextDialog(
            "Edit playlist", "Save", playlist.title, "Name",
            { name, about ->
                editing = false
                actions.write(Change.Playlist(playlist.id), Change.Library, done = "Saved") {
                    actions.graph.api.editPlaylist(playlist.id, name, about)
                }
            },
            { editing = false },
            second = playlist.description, secondLabel = "Description",
        )
    }
}

@Composable
fun LikedScreen(shuffle: Boolean = false) {
    val actions = LocalActions.current
    val loader = rememberLoader("liked", follows = setOf(Change.Liked)) { actions.graph.api.liked() }
    // A like is written and said a second later: what was unliked goes from the page at once.
    var unliked by remember { mutableStateOf(emptySet<String>()) }
    LaunchedEffect(actions.graph.liked) {
        var before = actions.graph.liked.ids.value
        actions.graph.liked.ids.collect { after ->
            unliked = unlikedSince(unliked, before, after)
            before = after
        }
    }
    var shuffled by rememberSaveable { mutableStateOf(false) }
    SubPage(loader, "Liked Music") { liked ->
        // Once, not each time the page is come back to.
        LaunchedEffect(shuffle) {
            if (shuffle && !shuffled) {
                shuffled = true
                actions.playback.shuffle(liked.tracks, "Liked Music")
            }
        }
        val tracks = stillLiked(liked.tracks, unliked)
        TrackList(liked.copy(title = "Liked Music"), "Liked Music", "Auto playlist", emptyList(), editable = false, loader = loader, tracks = tracks, liked = true)
    }
}

/** An episode: whether it is the one playing, and how far it was listened to, as a line and a bar. */
@Composable
private fun EpisodeRow(episode: Track, author: String, key: String, selection: TrackSelection, onPlay: () -> Unit, actions: Actions) {
    val context = LocalContext.current
    val p = LocalPalette.current
    val playingId by LocalPlayingId.current
    val position = if (episode.id == playingId) {
        val projection by actions.graph.playback.projection.collectAsStateWithLifecycle()
        projection.state.positionMs
    } else {
        remember(episode.id, playingId) { EpisodeProgress.of(context, episode.id) }
    }
    Column {
        SelectableRow(
            episode, key, selection, onPlay, { actions.menuFor(episode) },
            subtitle = episodeLine(position, episode.durationMs, clock(episode.durationMs).takeIf { episode.durationMs > 0 } ?: author),
        )
        val fraction = episodeFraction(position, episode.durationMs)
        if (fraction > 0f) {
            LinearProgressIndicator(
                progress = { fraction }, color = p.accent, trackColor = p.surface,
                modifier = Modifier.fillMaxWidth().padding(start = 76.dp, end = 16.dp).height(2.dp),
            )
        }
    }
}

@Composable
fun PodcastScreen(id: String) {
    val actions = LocalActions.current
    val loader = rememberLoader("podcast:$id") { actions.graph.api.podcast(id) }
    SubPage(loader) { podcast ->
        val list = rememberLazyListState()
        val keys = remember(podcast.episodes) { rowKeys(podcast.episodes) }
        val selection = rememberTrackSelection()
        Refreshable(loader) {
            Box(Modifier.fillMaxSize()) {
                LazyColumn(state = list, contentPadding = PaddingValues(bottom = if (selection.active) 140.dp else 24.dp)) {
                    collectionHeader(podcast.title, podcast.author, if (podcast.episodes.size == 1) "1 episode" else "${podcast.episodes.size} episodes", podcast.artwork, podcast.episodes, podcast.title, actions)
                    if (podcast.description.isNotEmpty()) {
                        item { ExpandableText(podcast.description, 6, Modifier.padding(16.dp), style = Type.small, color = LocalPalette.current.secondary) }
                    }
                    itemsIndexed(podcast.episodes) { i, e ->
                        EpisodeRow(e, podcast.author, keys[i], selection, { actions.playEpisode(podcast.episodes, i, podcast.title) }, actions)
                    }
                }
                FadingTopBar(podcast.title, artColor(podcast.artwork), list) { actions.nav.popBackStack() }
                TrackSelectionBar(selection, podcast.episodes, keys, actions)
            }
        }
    }
}

@Composable
fun HistoryScreen() {
    val actions = LocalActions.current
    val loader = rememberLoader("history") { actions.graph.api.history() }
    val selection = rememberTrackSelection()
    Column(Modifier.fillMaxSize()) {
        TitleBar("Recently played")
        Refreshable(loader) {
            Loaded(loader, skeleton = { SkeletonList() }) { tracks ->
                val keys = remember(tracks) { rowKeys(tracks) }
                Box(Modifier.fillMaxSize()) {
                    LazyColumn(contentPadding = PaddingValues(bottom = if (selection.active) 140.dp else 24.dp)) {
                        itemsIndexed(tracks, key = { i, _ -> keys[i] }) { i, t ->
                            SelectableRow(t, keys[i], selection, { actions.play(tracks, i, "Recently played") }, { actions.menuFor(t) })
                        }
                        if (tracks.isEmpty()) item { Empty("Nothing yet" to "What you play shows up here.") }
                    }
                    TrackSelectionBar(selection, tracks, keys, actions)
                }
            }
        }
    }
}

/** Any other surface: a mood's page, the whole of a shelf, Discover, the charts. */
@Composable
fun BrowseScreen(id: String, params: String, title: String) {
    val actions = LocalActions.current
    val loader = rememberLoader("browse:$id:$params") { actions.graph.api.browse(id, params) }
    val videos = PagePrefs.of(LocalContext.current).showMusicVideos
    val state by loader.state.collectAsStateWithLifecycle()
    val pageTitle = (state as? Load.Ready)?.value?.title.orEmpty().ifEmpty { title }
    Column(Modifier.fillMaxSize()) {
        TitleBar(pageTitle)
        Refreshable(loader) {
            Loaded(loader, skeleton = { SkeletonShelves() }) { page ->
                val shelves = remember(page, videos) { page.shelves.map { shelfShown(it, videos) }.filter { it.items.isNotEmpty() } }
                // The shelves that are lists of songs are chosen from as one list.
                val listed = remember(shelves) { shelves.map { s -> s.items.filterIsInstance<Item.Song>().map { it.track }.takeIf { it.size == s.items.size }.orEmpty() } }
                val songsListed = remember(listed) { listed.flatten() }
                val keys = remember(songsListed) { rowKeys(songsListed) }
                val firstKey = remember(listed) { listed.runningFold(0) { at, songs -> at + songs.size } }
                val selection = rememberTrackSelection()
                Box(Modifier.fillMaxSize()) {
                LazyColumn(contentPadding = PaddingValues(bottom = if (selection.active) 140.dp else 24.dp)) {
                    if (page.moods.isNotEmpty()) {
                        item {
                            LazyRow(contentPadding = PaddingValues(16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                                items(page.moods) { m -> Pill(m.title, { actions.go(Routes.browse(m.id, m.params, m.title)) }) }
                            }
                        }
                    }
                    itemsIndexed(shelves) { at, shelf ->
                        val heading = shelfHeading(shelf, pageTitle)
                        if (heading.isNotEmpty()) SectionTitle(heading)
                        val songs = shelf.items.filterIsInstance<Item.Song>().map { it.track }
                        val origin = shelf.title.ifEmpty { pageTitle }
                        if (songs.size == shelf.items.size) {
                            Column {
                                songs.forEachIndexed { i, t ->
                                    SelectableRow(t, keys[firstKey[at] + i], selection, { actions.play(songs, i, origin) }, { actions.menuFor(t) })
                                }
                            }
                        } else {
                            // A song plays its shelf's songs, as on Home.
                            ShelfRow(shelf.items, { item ->
                                if (item is Item.Song) actions.play(songs, songs.indexOf(item.track), origin) else actions.open(item)
                            }, actions::menuFor)
                        }
                    }
                    if (page.shelves.all { shelfShown(it, videos).items.isEmpty() } && page.moods.isEmpty()) {
                        item { Empty("Nothing here" to "This page came back empty. Pull down to try again.") }
                    }
                }
                TrackSelectionBar(selection, songsListed, keys, actions)
                }
            }
        }
    }
}

/** Above this many, liking a whole list asks first: each like is a request to YouTube. */
private const val LIKE_ASKS_ABOVE = 20
