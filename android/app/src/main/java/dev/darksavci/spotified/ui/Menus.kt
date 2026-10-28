package dev.darksavci.spotified.ui

import dev.darksavci.spotified.ui.components.ThinSlider
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectDragGesturesAfterLongPress
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import dev.darksavci.spotified.R
import dev.darksavci.spotified.api.Artwork
import dev.darksavci.spotified.api.Item
import dev.darksavci.spotified.api.LibraryItem
import dev.darksavci.spotified.api.LibraryKind
import dev.darksavci.spotified.api.Repeat
import dev.darksavci.spotified.api.Track
import dev.darksavci.spotified.data.Settings
import dev.darksavci.spotified.playback.SleepTimer
import dev.darksavci.spotified.ui.components.Art
import dev.darksavci.spotified.ui.components.Chip
import dev.darksavci.spotified.ui.components.Glyph
import dev.darksavci.spotified.ui.components.TrackRow
import dev.darksavci.spotified.ui.components.subtitleOf
import dev.darksavci.spotified.ui.theme.LocalPalette
import dev.darksavci.spotified.ui.theme.Type
import kotlinx.coroutines.launch

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun Menus() {
    val actions = LocalActions.current
    val target = actions.menu ?: return
    val p = LocalPalette.current
    val sheet = rememberModalBottomSheetState(skipPartiallyExpanded = target is MenuTarget.Queue)
    ModalBottomSheet(
        onDismissRequest = { actions.menu = null },
        sheetState = sheet,
        containerColor = p.panel,
        contentColor = p.text,
    ) {
        Column(Modifier.padding(bottom = 24.dp)) {
            when (target) {
                is MenuTarget.Song -> SongMenu(actions, target)
                is MenuTarget.Entity -> EntityMenu(actions, target.item)
                is MenuTarget.AddTo -> AddToPlaylist(actions, target.tracks)
                MenuTarget.SleepTimer -> SleepMenu(actions)
                MenuTarget.Speed -> SpeedMenu(actions)
                MenuTarget.Queue -> QueueSheet(actions)
            }
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
private fun Entry(icon: Int, label: String, onClick: () -> Unit) {
    Row(
        Modifier.fillMaxWidth().clickable(onClick = onClick).padding(horizontal = 20.dp, vertical = 14.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Glyph(icon, null, tint = LocalPalette.current.secondary)
        Spacer(Modifier.width(18.dp))
        Text(label, style = Type.body)
    }
}

@Composable
private fun SongMenu(actions: Actions, target: MenuTarget.Song) {
    val track = target.track
    val graph = actions.graph
    val liked by graph.liked.ids.collectAsStateWithLifecycle()
    val downloads by graph.downloads.done.collectAsStateWithLifecycle()
    val close = { actions.menu = null }
    Header(track.title, listOfNotNull(track.artistNames, track.album?.name).joinToString(" • "), track.artwork)
    if (track.id in liked) {
        Entry(R.drawable.ic_heart_filled, "Remove from Liked Music") { graph.liked.set(track.id, false); close() }
    } else {
        Entry(R.drawable.ic_heart, "Add to Liked Music") { graph.liked.set(track.id, true); close() }
    }
    Entry(R.drawable.ic_list_plus, "Play next") { actions.playback.playNext(listOf(track)); actions.say("Plays next"); close() }
    Entry(R.drawable.ic_list_end, "Add to queue") { actions.playback.addToQueue(listOf(track)); actions.say("Added to the queue"); close() }
    Entry(R.drawable.ic_circle_plus, "Add to playlist") { actions.addTo(listOf(track)) }
    Entry(R.drawable.ic_radio, "Start radio") { actions.playback.startRadio(track); close() }
    if (track.album?.id?.isNotEmpty() == true) Entry(R.drawable.ic_disc_3, "Go to album") { close(); actions.openAlbumOf(track) }
    if (track.artists.any { it.id.isNotEmpty() }) Entry(R.drawable.ic_user, "Go to artist") { close(); actions.openArtistOf(track) }
    if (track.id in downloads) {
        Entry(R.drawable.ic_trash_2, "Remove download") { graph.downloads.remove(listOf(track)); close() }
    } else {
        Entry(R.drawable.ic_archive, "Download") { graph.downloads.add(listOf(track)); actions.say("Downloading"); close() }
    }
    if (target.queueIndex != null) {
        Entry(R.drawable.ic_trash_2, "Remove from queue") { actions.playback.remove(target.queueIndex); close() }
    }
    val playlist = target.playlist
    if (playlist != null && playlist.editable && track.playlistItemId.isNotEmpty()) {
        Entry(R.drawable.ic_trash_2, "Remove from this playlist") {
            actions.launch {
                graph.api.removeFromPlaylist(playlist.id, listOf(track.id to track.playlistItemId))
                actions.say("Removed from ${playlist.title}")
            }
            close()
        }
    }
    Entry(R.drawable.ic_share_2, "Share") { close(); actions.share(track.title, Actions.songLink(track.id)) }
}

@Composable
private fun EntityMenu(actions: Actions, item: Item) {
    val close = { actions.menu = null }
    Header(item.title, subtitleOf(item), item.artwork)
    when (item) {
        is Item.AlbumItem -> {
            Entry(R.drawable.ic_play, "Play") { close(); actions.launch { actions.playAlbum(actions.graph.api.album(item.album.id)) } }
            Entry(R.drawable.ic_list_end, "Add to queue") {
                close(); actions.launch { actions.playback.addToQueue(actions.graph.api.album(item.album.id).tracks) }
            }
            Entry(R.drawable.ic_archive, "Download") {
                close(); actions.launch { actions.graph.downloads.add(actions.graph.api.album(item.album.id).tracks) }
            }
            Entry(R.drawable.ic_pin, "Pin in your library") {
                close(); actions.launch { actions.graph.api.organise(LibraryKind.Album, item.album.id, pinned = true) }
            }
            Entry(R.drawable.ic_share_2, "Share") { close(); actions.share(item.title, Actions.albumLink(item.album.id)) }
        }
        is Item.PlaylistItem -> {
            Entry(R.drawable.ic_play, "Play") {
                close(); actions.launch { actions.graph.api.completePlaylist(item.playlist.id).let { actions.play(it.tracks, 0, it.title) } }
            }
            Entry(R.drawable.ic_shuffle, "Shuffle") {
                close(); actions.launch { actions.graph.api.completePlaylist(item.playlist.id).let { actions.playback.shuffle(it.tracks, it.title) } }
            }
            Entry(R.drawable.ic_archive, "Download") {
                close(); actions.launch { actions.graph.downloads.addPlaylist(item.playlist.id) }
            }
            Entry(R.drawable.ic_pin, "Pin in your library") {
                close(); actions.launch { actions.graph.api.organise(LibraryKind.Playlist, item.playlist.id, pinned = true) }
            }
            Entry(R.drawable.ic_share_2, "Share") { close(); actions.share(item.title, Actions.playlistLink(item.playlist.id)) }
        }
        is Item.ArtistItem -> {
            Entry(R.drawable.ic_radio, "Start radio") {
                close(); actions.launch {
                    val a = actions.graph.api.artist(item.artist.id)
                    if (a.radioId.isNotEmpty()) actions.playback.startMix(a.radioId, a.radioSeed, a.radioParams, "${a.name} radio")
                }
            }
            Entry(R.drawable.ic_share_2, "Share") { close(); actions.share(item.title, Actions.artistLink(item.artist.id)) }
        }
        is Item.PodcastItem -> {
            Entry(R.drawable.ic_play, "Open") { close(); actions.open(item) }
        }
        else -> Unit
    }
}

@Composable
private fun AddToPlaylist(actions: Actions, tracks: List<Track>) {
    val graph = actions.graph
    val p = LocalPalette.current
    val playlists by produceState(emptyList<LibraryItem>()) {
        value = runCatching { graph.api.library().filter { it.kind == LibraryKind.Playlist } }.getOrDefault(emptyList())
    }
    var naming by remember { mutableStateOf(false) }
    var name by remember { mutableStateOf("") }
    val close = { actions.menu = null }
    Text("Add to playlist", style = Type.heading, modifier = Modifier.padding(16.dp))
    if (naming) {
        OutlinedTextField(name, { name = it }, Modifier.fillMaxWidth().padding(horizontal = 16.dp), label = { Text("Name") }, singleLine = true)
        Row(Modifier.fillMaxWidth().padding(16.dp), horizontalArrangement = Arrangement.End) {
            Text(
                "Create", style = Type.bodyStrong, color = p.accent,
                modifier = Modifier.clip(RoundedCornerShape(16.dp)).clickable(enabled = name.isNotBlank()) {
                    actions.launch {
                        val id = graph.api.createPlaylist(name.trim())
                        graph.api.addToPlaylist(id, tracks.map { it.id })
                        actions.say("Added to ${name.trim()}")
                    }
                    close()
                }.padding(12.dp),
            )
        }
        return
    }
    Entry(R.drawable.ic_plus, "New playlist") { naming = true }
    LazyColumn(Modifier.heightIn(max = 420.dp)) {
        itemsIndexed(playlists) { _, list ->
            Row(
                Modifier.fillMaxWidth().clickable {
                    actions.launch {
                        graph.api.addToPlaylist(list.id, tracks.map { it.id })
                        actions.say("Added to ${list.title}")
                    }
                    close()
                }.padding(horizontal = 16.dp, vertical = 8.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Art(list.artwork, 48.dp)
                Spacer(Modifier.width(12.dp))
                Text(list.title, style = Type.body, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
        }
    }
}

@Composable
private fun SleepMenu(actions: Actions) {
    val sleep = actions.graph.sleep
    val endsAt by sleep.endsAt.collectAsStateWithLifecycle()
    val close = { actions.menu = null }
    Text("Sleep timer", style = Type.heading, modifier = Modifier.padding(16.dp))
    endsAt?.let { at ->
        val left = if (at == SleepTimer.END_OF_SONG) "at the end of this song" else "in ${((at - System.currentTimeMillis()) / 60_000) + 1} min"
        Text("Stops $left", style = Type.body, color = LocalPalette.current.secondary, modifier = Modifier.padding(horizontal = 16.dp))
        Entry(R.drawable.ic_ban, "Turn off the timer") { sleep.cancel(); close() }
    }
    for (minutes in listOf(5, 10, 15, 30, 45, 60)) {
        Entry(R.drawable.ic_clock, "$minutes minutes") { sleep.after(minutes); close() }
    }
    Entry(R.drawable.ic_music, "End of this song") { sleep.atEndOfSong(); close() }
}

@Composable
private fun SpeedMenu(actions: Actions) {
    val graph = actions.graph
    val scope = rememberCoroutineScope()
    val settings by graph.settings.settings.collectAsStateWithLifecycle(Settings())
    var speed by remember(settings.speed) { mutableFloatStateOf(settings.speed) }
    val p = LocalPalette.current
    Text("Speed: %.2f×".format(speed), style = Type.heading, modifier = Modifier.padding(16.dp))
    ThinSlider(
        speed, { speed = (it * 20).toInt() / 20f },
        { scope.launch { graph.settings.update { it.copy(speed = speed) } } },
        0.5f..2f, Modifier.padding(horizontal = 16.dp),
    )
    Row(Modifier.padding(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        for (preset in listOf(0.75f, 1f, 1.25f, 1.5f)) {
            Chip("%.2f×".format(preset), speed == preset, {
                speed = preset
                scope.launch { graph.settings.update { it.copy(speed = preset) } }
            })
        }
    }
}

/** The queue: what plays now, then what is next, dragged by the handle to reorder and swiped to the menu to remove. */
@Composable
private fun QueueSheet(actions: Actions) {
    val graph = actions.graph
    val projection by graph.playback.projection.collectAsStateWithLifecycle()
    val queue = projection.state.queue
    val p = LocalPalette.current
    val list = rememberLazyListState()
    var dragFrom by remember { mutableIntStateOf(-1) }
    var dragBy by remember { mutableFloatStateOf(0f) }
    Text("Queue", style = Type.heading, modifier = Modifier.padding(start = 16.dp, top = 8.dp, bottom = 4.dp))
    // What plays, then what comes, as Spotify's queue reads; what was played is behind the previous button.
    val current = queue.items.getOrNull(queue.index)
    val upcoming = queue.items.withIndex().drop(queue.index + 1)
    LazyColumn(state = list, modifier = Modifier.heightIn(max = 560.dp)) {
        if (current != null) {
            item { QueueHeading("Now playing") }
            item(key = "now:${current.id}") {
                TrackRow(track = current, playing = true, onClick = {}, onMenu = { actions.menuFor(current, queueIndex = queue.index) })
            }
        }
        if (upcoming.isNotEmpty()) item { QueueHeading("Next from: ${queue.origin.ifEmpty { "your queue" }}") }
        items(upcoming, key = { (i, t) -> "$i:${t.id}" }) { (i, track) ->
            val dragging = i == dragFrom
            TrackRow(
                track = track,
                playing = i == queue.index,
                onClick = { actions.playback.jump(i) },
                onMenu = { actions.menuFor(track, queueIndex = i) },
                modifier = Modifier
                    .background(if (dragging) p.surface else Color.Transparent)
                    .pointerInput(i, queue.items.size) {
                        detectDragGesturesAfterLongPress(
                            onDragStart = { dragFrom = i; dragBy = 0f },
                            onDragEnd = {
                                val rows = (dragBy / 64.dp.toPx()).toInt()
                                val to = (dragFrom + rows).coerceIn(0, queue.items.size - 1)
                                if (to != dragFrom) actions.playback.move(dragFrom, to)
                                dragFrom = -1
                            },
                            onDragCancel = { dragFrom = -1 },
                        ) { change, amount -> change.consume(); dragBy += amount.y }
                    },
            )
        }
    }
    Row(Modifier.fillMaxWidth().padding(16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        val s = projection.state
        QueueButton(R.drawable.ic_shuffle, "Shuffle", s.shuffle, Modifier.weight(1f)) { actions.playback.setShuffle(!s.shuffle) }
        QueueButton(if (s.repeat == Repeat.One) R.drawable.ic_repeat_1 else R.drawable.ic_repeat, if (s.repeat == Repeat.One) "Repeat one" else "Repeat", s.repeat != Repeat.Off, Modifier.weight(1f)) { actions.playback.cycleRepeat() }
        QueueButton(R.drawable.ic_clock, "Timer", graph.sleep.endsAt.value != null, Modifier.weight(1f)) { actions.menu = MenuTarget.SleepTimer }
    }
}

@Composable
private fun QueueHeading(text: String) {
    Text(text, style = Type.bodyStrong.copy(fontSize = 16.sp), maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(start = 16.dp, end = 16.dp, top = 16.dp, bottom = 4.dp))
}

@Composable
private fun QueueButton(icon: Int, label: String, on: Boolean, modifier: Modifier, onClick: () -> Unit) {
    val p = LocalPalette.current
    Column(
        modifier.clip(RoundedCornerShape(8.dp)).background(p.surface).clickable(onClick = onClick).padding(vertical = 10.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Glyph(icon, null, tint = if (on) p.accent else p.text, size = 20.dp)
        Text(label, style = Type.tiny, color = if (on) p.accent else p.text)
    }
}
