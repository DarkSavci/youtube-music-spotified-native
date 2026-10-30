package dev.darksavci.spotified.ui

import dev.darksavci.spotified.api.ArtistRef
import androidx.compose.animation.animateContentSize
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Snackbar
import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.SnackbarResult
import androidx.compose.material3.SheetValue
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import dev.darksavci.spotified.R
import dev.darksavci.spotified.api.Artwork
import dev.darksavci.spotified.api.Command
import dev.darksavci.spotified.api.Item
import dev.darksavci.spotified.api.LibraryItem
import dev.darksavci.spotified.api.LibraryKind
import dev.darksavci.spotified.api.Track
import dev.darksavci.spotified.data.BlockKind
import dev.darksavci.spotified.data.Blocked
import dev.darksavci.spotified.data.Services
import dev.darksavci.spotified.data.Settings
import dev.darksavci.spotified.data.blocked
import dev.darksavci.spotified.data.learnBlockedAlbums
import dev.darksavci.spotified.data.setBlocked
import dev.darksavci.spotified.playback.SleepTimer
import dev.darksavci.spotified.ui.components.Art
import dev.darksavci.spotified.ui.components.Chip
import dev.darksavci.spotified.ui.components.Glyph
import dev.darksavci.spotified.ui.components.Loading
import dev.darksavci.spotified.ui.components.Problem
import dev.darksavci.spotified.ui.components.ThinSlider
import dev.darksavci.spotified.ui.components.subtitleOf
import dev.darksavci.spotified.ui.components.tapRow
import dev.darksavci.spotified.ui.components.unplayableReason
import dev.darksavci.spotified.ui.player.QueueSheet
import dev.darksavci.spotified.ui.screens.FolderSheet
import dev.darksavci.spotified.ui.screens.bareId
import dev.darksavci.spotified.ui.screens.libraryEntry
import dev.darksavci.spotified.ui.screens.moveTargetOf
import dev.darksavci.spotified.ui.screens.setSaved
import dev.darksavci.spotified.ui.player.countdown
import dev.darksavci.spotified.ui.theme.LocalPalette
import dev.darksavci.spotified.ui.theme.Type
import java.util.Locale
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.flow.filter
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.merge
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/**
 * The one sheet every menu shows in. It stays the same sheet as one menu leads to
 * another, resizing rather than rebuilding, and closes by sliding away. A menu's
 * messages show in it too, since Root's would be hidden under it.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun Menus() {
    val actions = LocalActions.current
    val p = LocalPalette.current
    val sheet = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    var shown by remember { mutableStateOf(actions.menu) }
    val messages = remember { SnackbarHostState() }

    LaunchedEffect(actions.menu) {
        val next = actions.menu
        if (next != null) {
            // A menu dismissed by a swipe has hidden the sheet, and the one it goes back to brings it out again.
            val reopening = shown != null && (sheet.currentValue != SheetValue.Expanded || sheet.targetValue != SheetValue.Expanded)
            shown = next
            if (reopening) sheet.show()
        } else if (shown != null) {
            // However it was closed, nothing is left to come back to.
            actions.closeMenus()
            sheet.hide()
            shown = null
        }
    }
    LaunchedEffect(actions) {
        merge(
            actions.graph.playback.messages.filter { it.isNotBlank() }.map { Notice(it) },
            actions.notices,
        ).collectLatest { notice ->
            if (shown == null) return@collectLatest
            val duration = if (notice.action != null) SnackbarDuration.Long else SnackbarDuration.Short
            if (messages.showSnackbar(notice.message, notice.action, duration = duration) == SnackbarResult.ActionPerformed) notice.run?.invoke()
        }
    }

    val target = shown ?: return
    ModalBottomSheet(
        onDismissRequest = { actions.closeMenu() },
        sheetState = sheet,
        containerColor = p.panel,
        contentColor = p.text,
        contentWindowInsets = { WindowInsets(0) },
    ) {
        Box {
            Column(Modifier.animateContentSize()) {
                if (target is MenuTarget.Queue) {
                    QueueSheet(actions)
                } else {
                    ScrollingMenu {
                        when (target) {
                            is MenuTarget.Song -> SongMenu(actions, target)
                            is MenuTarget.Entity -> EntityMenu(actions, target.item)
                            is MenuTarget.AddTo -> AddToPlaylist(actions, target.tracks)
                            MenuTarget.SleepTimer -> SleepMenu(actions)
                            MenuTarget.Speed -> SpeedMenu(actions)
                            MenuTarget.Create -> CreateMenu(actions)
                            MenuTarget.NewPlaylist -> NamePlaylist(actions, emptyList())
                            is MenuTarget.Share -> ShareMenu(actions, target.what)
                            is MenuTarget.Block -> BlockMenu(actions, target.track)
                            is MenuTarget.Artists -> ArtistsMenu(actions, target.track.artists)
                            is MenuTarget.ArtistsOf -> ArtistsMenu(actions, target.artists)
                            is MenuTarget.MoveToFolder -> FolderSheet(actions, target)
                            MenuTarget.Queue -> Unit
                        }
                    }
                }
            }
            SnackbarHost(messages, Modifier.align(Alignment.BottomCenter).windowInsetsPadding(WindowInsets.navigationBars)) { data ->
                Snackbar(data, containerColor = p.overlay, contentColor = p.text, actionColor = p.accent)
            }
        }
    }
}

/**
 * A menu's entries, as tall as they need up to most of the screen. A menu taller
 * than that scrolls, and its foot fades while there is more below, so that it is
 * clear there is more. The last entry is clear of the gesture bar.
 */
@Composable
private fun ScrollingMenu(content: @Composable () -> Unit) {
    val scroll = rememberScrollState()
    val panel = LocalPalette.current.panel
    Box(Modifier.heightIn(max = LocalConfiguration.current.screenHeightDp.dp * 0.85f)) {
        Column(Modifier.verticalScroll(scroll).padding(bottom = 16.dp).windowInsetsPadding(WindowInsets.navigationBars).imePadding()) {
            content()
        }
        if (scroll.canScrollForward) {
            Box(
                Modifier.align(Alignment.BottomCenter).fillMaxWidth().height(56.dp)
                    .background(Brush.verticalGradient(listOf(Color.Transparent, panel))),
            )
        }
    }
}

@Composable
private fun Header(title: String, subtitle: String, artwork: List<Artwork>) {
    Row(Modifier.padding(horizontal = 16.dp, vertical = 8.dp), verticalAlignment = Alignment.CenterVertically) {
        Art(artwork, 48.dp)
        Spacer(Modifier.width(12.dp))
        Column {
            Text(title, style = Type.bodyStrong, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text(subtitle, style = Type.small, color = LocalPalette.current.secondary, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
    }
    Spacer(Modifier.height(8.dp))
}

@Composable
private fun Entry(icon: Int, label: String, onClick: () -> Unit) = Entry(icon, label, null, onClick)

@Composable
private fun Entry(icon: Int, label: String, detail: String?, onClick: () -> Unit) {
    Row(
        Modifier.fillMaxWidth().tapRow(onClick = onClick, role = Role.Button).padding(horizontal = 20.dp, vertical = 14.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Glyph(icon, null, tint = LocalPalette.current.secondary)
        Spacer(Modifier.width(18.dp))
        Column {
            Text(label, style = Type.body, maxLines = 2, overflow = TextOverflow.Ellipsis)
            if (detail != null) Text(detail, style = Type.small, color = LocalPalette.current.secondary)
        }
    }
}

@Composable
private fun SongMenu(actions: Actions, target: MenuTarget.Song) {
    val track = target.track
    val graph = actions.graph
    val liked by graph.liked.ids.collectAsStateWithLifecycle()
    val downloads by graph.downloads.done.collectAsStateWithLifecycle()
    val done = { actions.finishMenu() }
    // The menu's words stay in Search's and Library's order: what to do with it, where to go from it, then the rest.
    Header(track.title, unplayableReason(track) ?: listOfNotNull(track.artistNames, track.album?.name).joinToString(" • "), track.artwork)
    if (track.id in liked) {
        Entry(R.drawable.ic_heart_filled, "Remove from Liked Music") { graph.liked.set(track.id, false); done() }
    } else {
        Entry(R.drawable.ic_heart, "Add to Liked Music") { graph.liked.set(track.id, true); done() }
    }
    Entry(R.drawable.ic_list_plus, "Play next") { actions.playback.playNext(listOf(track)); actions.say("Plays next"); done() }
    Entry(R.drawable.ic_list_end, "Add to queue") { actions.playback.addToQueue(listOf(track)); actions.say("Added to the queue"); done() }
    if (target.queueIndex == null) Entry(R.drawable.ic_list_music, "Go to queue") { actions.closeMenus(); actions.menu = MenuTarget.Queue }
    Entry(R.drawable.ic_circle_plus, "Add to playlist") { actions.addTo(listOf(track)) }
    Entry(R.drawable.ic_radio, "Start radio") { actions.playback.startRadio(track); done() }
    if (track.album?.id?.isNotEmpty() == true) Entry(R.drawable.ic_disc_3, "Go to album") { actions.closeMenus(); actions.openAlbumOf(track) }
    // Every artist that has a page, not only the first: a song by two is as much the second's.
    for (artist in track.artists.filter { it.id.isNotEmpty() }) {
        Entry(R.drawable.ic_user, "Go to ${artist.name}") { actions.closeMenus(); actions.go(Routes.artist(artist.id)) }
    }
    if (track.id in downloads) {
        Entry(R.drawable.ic_trash_2, "Remove download") { graph.downloads.remove(listOf(track)); done() }
    } else {
        Entry(R.drawable.ic_download, "Download") { graph.downloads.add(listOf(track)); actions.say("Downloading"); done() }
    }
    val place = target.queueIndex
    if (place != null) {
        Entry(R.drawable.ic_trash_2, "Remove from queue") {
            done()
            actions.playback.remove(place)
            actions.sayWithUndo("Removed ${track.title}") { actions.playback.send(Command.Enqueue(listOf(track), place)) }
        }
    }
    val playlist = target.playlist
    if (playlist != null && playlist.editable && track.playlistItemId.isNotEmpty()) {
        Entry(R.drawable.ic_trash_2, "Remove from this playlist") {
            done()
            actions.write(Change.Playlist(playlist.id), Change.Library) {
                graph.api.removeFromPlaylist(playlist.id, listOf(track.id to track.playlistItemId))
            }
            actions.sayWithUndo("Removed from ${playlist.title}") {
                actions.write(Change.Playlist(playlist.id), Change.Library) { graph.api.addToPlaylist(playlist.id, listOf(track.id)) }
            }
        }
    }
    Entry(R.drawable.ic_ban, "Block…") { actions.menuFrom(target, MenuTarget.Block(track)) }
    Entry(R.drawable.ic_share_2, "Share") { actions.menuFrom(target, MenuTarget.Share(Shared.Song(track))) }
}

@Composable
private fun EntityMenu(actions: Actions, item: Item) {
    val close = { actions.finishMenu() }
    val api = actions.graph.api
    val target = MenuTarget.Entity(item)
    Header(item.title, subtitleOf(item), item.artwork)
    moveTargetOf(item)?.let { move -> Entry(R.drawable.ic_folder, "Move to folder") { actions.menuFrom(target, move) } }
    when (item) {
        is Item.AlbumItem -> {
            val id = item.album.id
            Entry(R.drawable.ic_play, "Play") { close(); actions.launch { actions.playAlbum(api.album(id)) } }
            Entry(R.drawable.ic_list_end, "Add to queue") { close(); actions.launch { actions.playback.addToQueue(api.album(id).tracks) } }
            Entry(R.drawable.ic_heart, "Like all") { close(); likeAll(actions) { api.album(id).tracks } }
            Entry(R.drawable.ic_download, "Download") { close(); actions.launch { actions.graph.downloads.add(api.album(id).tracks) } }
            SaveEntry(actions, LibraryKind.Album, id, item.title)
            PinEntry(actions, LibraryKind.Album, id)
            BlockEntry(actions, BlockKind.Album, id, item.title, "Block this album", "Unblock this album")
            Entry(R.drawable.ic_share_2, "Share") { actions.menuFrom(target, MenuTarget.Share(Shared.AlbumOf(item.album))) }
        }
        is Item.PlaylistItem -> {
            val id = item.playlist.id
            Entry(R.drawable.ic_play, "Play") { close(); actions.launch { api.completePlaylist(id).let { actions.play(it.tracks, 0, it.title) } } }
            Entry(R.drawable.ic_shuffle, "Shuffle") { close(); actions.launch { api.completePlaylist(id).let { actions.playback.shuffle(it.tracks, it.title) } } }
            Entry(R.drawable.ic_heart, "Like all") { close(); likeAll(actions) { api.completePlaylist(id).tracks } }
            Entry(R.drawable.ic_download, "Download") { close(); actions.launch { actions.graph.downloads.addPlaylist(id) } }
            SaveEntry(actions, LibraryKind.Playlist, id, item.title)
            PinEntry(actions, LibraryKind.Playlist, id)
            Entry(R.drawable.ic_share_2, "Share") { actions.menuFrom(target, MenuTarget.Share(Shared.PlaylistOf(id, item.title))) }
        }
        is Item.ArtistItem -> {
            Entry(R.drawable.ic_radio, "Start radio") {
                close(); actions.launch {
                    val a = api.artist(item.artist.id)
                    if (a.radioId.isNotEmpty()) actions.playback.startMix(a.radioId, a.radioSeed, a.radioParams, "${a.name} radio")
                }
            }
            PinEntry(actions, LibraryKind.Artist, item.artist.id)
            BlockEntry(actions, BlockKind.Artist, item.artist.id, item.title, "Block ${item.title}", "Unblock ${item.title}")
            Entry(R.drawable.ic_share_2, "Share") { actions.menuFrom(target, MenuTarget.Share(Shared.ArtistOf(item.artist.id, item.title))) }
        }
        is Item.PodcastItem -> {
            Entry(R.drawable.ic_play, "Open") { actions.closeMenus(); actions.open(item) }
            PinEntry(actions, LibraryKind.Podcast, item.podcast.id)
        }
        else -> Unit
    }
}

/** Likes every song of a list that is not liked already, with a way back. */
private fun likeAll(actions: Actions, tracks: suspend () -> List<Track>) {
    val liked = actions.graph.liked
    actions.launch {
        val fresh = tracks().filter { it.id.isNotEmpty() && !liked.isLiked(it.id) }
        if (fresh.isEmpty()) {
            actions.say("All of these are liked already")
            return@launch
        }
        fresh.forEach { liked.set(it.id, true) }
        actions.sayWithUndo(if (fresh.size == 1) "Liked 1 song" else "Liked ${fresh.size} songs") {
            fresh.forEach { liked.set(it.id, false) }
        }
    }
}

/** The library knows a playlist by its id without the browse id's "VL", and an artist by its channel. */
private fun libraryId(id: String) = id.removePrefix("VL").removePrefix("MPLA")

/**
 * Pin, or Unpin when it is pinned. Pins are the library's, so this shows only for
 * something in the library, once the library has said what it holds.
 */
@Composable
private fun PinEntry(actions: Actions, kind: LibraryKind, id: String) {
    val library by produceState<List<LibraryItem>?>(null, id) {
        value = runCatching { actions.graph.api.library() }.getOrNull()
    }
    val entry = library?.firstOrNull { it.kind == kind && libraryId(it.id) == libraryId(id) } ?: return
    val set = { pinned: Boolean ->
        actions.finishMenu()
        actions.write(Change.Library, done = if (pinned) "Pinned" else "Unpinned") { actions.graph.api.organise(kind, entry.id, pinned = pinned) }
    }
    if (entry.pinned) Entry(R.drawable.ic_pin_off, "Unpin") { set(false) }
    else Entry(R.drawable.ic_pin, "Pin in your library") { set(true) }
}

/**
 * Save, or Remove when it is in the library, once the library has said which.
 * Liked Music and a playlist of one's own are not saved or taken out: they are
 * there by being one's own, and deleting one is for its page.
 */
@Composable
private fun SaveEntry(actions: Actions, kind: LibraryKind, id: String, title: String) {
    if (id.isEmpty() || bareId(id) == "LM") return
    val api = actions.graph.api
    val saved by produceState<Boolean?>(null, kind, id) {
        value = runCatching {
            val own = kind == LibraryKind.Playlist && api.playlistPage(id).playlist.editable
            if (own) null else libraryEntry(api.library(), kind, id) != null
        }.getOrNull()
    }
    val inLibrary = saved ?: return
    Entry(if (inLibrary) R.drawable.ic_check else R.drawable.ic_circle_plus, if (inLibrary) "Remove from your library" else "Save to your library") {
        actions.finishMenu()
        setSaved(actions, kind, id, title, !inLibrary)
    }
}

/** The entry that blocks something, or lets what is blocked play again. Nothing without an id has one. */
@Composable
private fun BlockEntry(actions: Actions, kind: BlockKind, id: String, name: String, block: String, unblock: String) {
    if (id.isEmpty()) return
    val settings = actions.graph.settings
    val blocked by settings.blocked.collectAsStateWithLifecycle(Blocked())
    val isBlocked = blocked.has(kind, id)
    Entry(R.drawable.ic_ban, if (isBlocked) unblock else block) {
        actions.finishMenu()
        actions.launch {
            settings.setBlocked(kind, id, name, !isBlocked)
            // An album's songs are what the core is told, and they are on its page.
            if (kind == BlockKind.Album && !isBlocked) settings.learnBlockedAlbums(actions.graph.api)
        }
        if (isBlocked) {
            actions.say("Unblocked $name")
        } else {
            actions.sayWithUndo("Blocked $name") { actions.launch { settings.setBlocked(kind, id, name, false) } }
        }
    }
}

/** A song's blocks: itself, each artist that has a page, and its album. */
@Composable
private fun BlockMenu(actions: Actions, track: Track) {
    Text("Block", style = Type.heading, modifier = Modifier.padding(16.dp))
    BlockEntry(actions, BlockKind.Song, track.id, track.title, "Block this song", "Unblock this song")
    for (artist in track.artists) {
        BlockEntry(actions, BlockKind.Artist, artist.id, artist.name, "Block ${artist.name}", "Unblock ${artist.name}")
    }
    track.album?.let { BlockEntry(actions, BlockKind.Album, it.id, it.name, "Block this album", "Unblock this album") }
}

/** A song by several artists, from the name in the player: which one to go to. */
@Composable
private fun ArtistsMenu(actions: Actions, artists: List<ArtistRef>) {
    Text("Go to artist", style = Type.heading, modifier = Modifier.padding(16.dp))
    for (artist in artists.filter { it.id.isNotEmpty() }) {
        Entry(R.drawable.ic_user, artist.name) { actions.closeMenus(); actions.go(Routes.artist(artist.id)) }
    }
}

/** The links there are: this app's own, those on the other services, and the phone's share sheet. */
@Composable
private fun ShareMenu(actions: Actions, what: Shared) {
    val (title, link) = when (what) {
        is Shared.Song -> what.track.title to Actions.songLink(what.track.id)
        is Shared.AlbumOf -> what.album.title to Actions.albumLink(what.album.id)
        is Shared.ArtistOf -> what.name to Actions.artistLink(what.id)
        is Shared.PlaylistOf -> what.title to Actions.playlistLink(what.id)
    }
    val done = { actions.finishMenu() }
    Text("Share", style = Type.heading, modifier = Modifier.padding(start = 16.dp, end = 16.dp, top = 16.dp))
    Text(title, style = Type.small, color = LocalPalette.current.secondary, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(start = 16.dp, end = 16.dp, bottom = 8.dp))
    Entry(R.drawable.ic_copy, "Copy link") { done(); actions.copy(link, "Link copied to clipboard") }
    if (what !is Shared.PlaylistOf) {
        for (service in Services.Service.entries) {
            Entry(R.drawable.ic_copy, "${service.label} link") {
                done()
                when (what) {
                    is Shared.Song -> actions.copyLinkOn(service, what.track)
                    is Shared.AlbumOf -> actions.copyLinkOn(service, what.album)
                    is Shared.ArtistOf -> actions.copyLinkOn(service, Services.Wanted.Artist(what.name, what.id))
                    is Shared.PlaylistOf -> Unit
                }
            }
        }
    }
    Entry(R.drawable.ic_share_2, "Share via…") { done(); actions.share(title, link) }
}

@Composable
private fun AddToPlaylist(actions: Actions, tracks: List<Track>) {
    val graph = actions.graph
    var attempt by remember { mutableStateOf(0) }
    val playlists by produceState<Result<List<LibraryItem>>?>(null, attempt) {
        value = null
        // Liked Music is added to by liking, not as a playlist.
        value = runCatching { graph.api.library().filter { it.kind == LibraryKind.Playlist && it.id != "LM" } }
    }
    var naming by remember { mutableStateOf(false) }
    if (naming) {
        NamePlaylist(actions, tracks)
        return
    }
    Text("Add to playlist", style = Type.heading, modifier = Modifier.padding(16.dp))
    Entry(R.drawable.ic_plus, "New playlist") { naming = true }
    val loaded = playlists
    when {
        loaded == null -> Loading(Modifier.heightIn(max = 120.dp))
        loaded.isFailure -> Problem(loaded.exceptionOrNull()?.message ?: "Your playlists could not be loaded.", { attempt++ })
        else -> for (list in loaded.getOrThrow()) {
            Row(
                Modifier.fillMaxWidth().tapRow(
                    onClick = {
                        actions.finishMenu()
                        actions.write(Change.Playlist(list.id), Change.Library, done = "Added to ${list.title}") {
                            graph.api.addToPlaylist(list.id, tracks.map { it.id })
                        }
                    },
                    role = Role.Button,
                ).padding(horizontal = 16.dp, vertical = 8.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Art(list.artwork, 48.dp)
                Spacer(Modifier.width(12.dp))
                Text(list.title, style = Type.body, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
        }
    }
}

/** Asks a new playlist's name, makes it with any [tracks], and opens it. */
@Composable
private fun NamePlaylist(actions: Actions, tracks: List<Track>) {
    val graph = actions.graph
    val p = LocalPalette.current
    // A song's own name is what a playlist begun from it is offered as, as on the desktop.
    var name by remember { mutableStateOf(tracks.singleOrNull()?.title.orEmpty()) }
    val focus = remember { FocusRequester() }
    LaunchedEffect(Unit) { focus.requestFocus() }
    val create = {
        val title = name.trim()
        if (title.isNotEmpty()) {
            actions.closeMenus()
            actions.write(Change.Library, done = if (tracks.isEmpty()) "Created $title" else "Added to $title") {
                val id = graph.api.createPlaylist(title)
                if (tracks.isNotEmpty()) graph.api.addToPlaylist(id, tracks.map { it.id })
                withContext(Dispatchers.Main.immediate) { actions.go(Routes.playlist(id)) }
            }
        }
    }
    Text("New playlist", style = Type.heading, modifier = Modifier.padding(16.dp))
    OutlinedTextField(
        name, { name = it },
        Modifier.fillMaxWidth().padding(horizontal = 16.dp).focusRequester(focus),
        label = { Text("Name") }, singleLine = true,
        keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Sentences, imeAction = ImeAction.Done),
        keyboardActions = androidx.compose.foundation.text.KeyboardActions(onDone = { create() }),
    )
    Row(Modifier.fillMaxWidth().padding(16.dp), horizontalArrangement = Arrangement.End) {
        Text(
            "Create", style = Type.bodyStrong, color = if (name.isNotBlank()) p.accent else p.dim,
            modifier = Modifier.clip(RoundedCornerShape(16.dp)).tapRow(onClick = { create() }, enabled = name.isNotBlank(), role = Role.Button).padding(12.dp),
        )
    }
}

/** What the Create tab makes: a playlist, or one brought over from elsewhere. */
@Composable
private fun CreateMenu(actions: Actions) {
    Text("Create", style = Type.heading, modifier = Modifier.padding(16.dp))
    Entry(R.drawable.ic_list_music, "Playlist", "Build a playlist with songs") { actions.menu = MenuTarget.NewPlaylist }
    Entry(R.drawable.ic_list_plus, "Import a playlist", "From Spotify, Apple Music or Deezer") { actions.closeMenus(); actions.go(Routes.IMPORT) }
    Entry(R.drawable.ic_users, "Listen Together", "Play in step with friends") { actions.closeMenus(); actions.go(Routes.TOGETHER) }
}

/** Stays open once set, to show the time counting down and to change or turn it off. */
@Composable
private fun SleepMenu(actions: Actions) {
    val sleep = actions.graph.sleep
    val endsAt by sleep.endsAt.collectAsStateWithLifecycle()
    var now by remember { mutableLongStateOf(System.currentTimeMillis()) }
    LaunchedEffect(endsAt) {
        while (endsAt != null) {
            now = System.currentTimeMillis()
            delay(1000)
        }
    }
    Text("Sleep timer", style = Type.heading, modifier = Modifier.padding(16.dp))
    endsAt?.let { at ->
        val left = if (at == SleepTimer.END_OF_SONG) "at the end of this song" else "in ${countdown(at - now)}"
        Text("Stops $left", style = Type.body, color = LocalPalette.current.accent, modifier = Modifier.padding(horizontal = 16.dp))
        Entry(R.drawable.ic_ban, "Turn off the timer") { sleep.cancel() }
    }
    for (minutes in listOf(5, 10, 15, 30, 45, 60)) {
        Entry(R.drawable.ic_clock, "$minutes minutes") { sleep.after(minutes) }
    }
    Entry(R.drawable.ic_music, "End of this song") { sleep.atEndOfSong() }
}

@Composable
private fun SpeedMenu(actions: Actions) {
    val graph = actions.graph
    val scope = rememberCoroutineScope()
    val settings by graph.settings.settings.collectAsStateWithLifecycle(Settings())
    var speed by remember(settings.speed) { mutableFloatStateOf(settings.speed) }
    Text("Speed: %.2f×".format(Locale.US, speed), style = Type.heading, modifier = Modifier.padding(16.dp))
    ThinSlider(
        speed, { speed = (it * 20).toInt() / 20f },
        { scope.launch { graph.settings.update { it.copy(speed = speed) } } },
        0.5f..2f, Modifier.padding(horizontal = 16.dp), description = "Speed",
    )
    Row(Modifier.padding(16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        for (preset in listOf(0.75f, 1f, 1.25f, 1.5f)) {
            Chip("%.2f×".format(Locale.US, preset), speed == preset, {
                speed = preset
                scope.launch { graph.settings.update { it.copy(speed = preset) } }
            })
        }
    }
}
