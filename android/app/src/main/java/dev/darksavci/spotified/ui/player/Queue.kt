package dev.darksavci.spotified.ui.player

import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectVerticalDragGestures
import androidx.compose.foundation.gestures.scrollBy
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListItemInfo
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.SwipeToDismissBox
import androidx.compose.material3.SwipeToDismissBoxValue
import androidx.compose.material3.Text
import androidx.compose.material3.rememberSwipeToDismissBoxState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.zIndex
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import dev.darksavci.spotified.R
import dev.darksavci.spotified.api.Command
import dev.darksavci.spotified.api.Queue
import dev.darksavci.spotified.api.Repeat
import dev.darksavci.spotified.api.Track
import dev.darksavci.spotified.ui.Actions
import dev.darksavci.spotified.ui.MenuTarget
import dev.darksavci.spotified.ui.components.Art
import dev.darksavci.spotified.ui.components.Glyph
import dev.darksavci.spotified.ui.components.GlyphButton
import dev.darksavci.spotified.ui.components.rememberHaptics
import dev.darksavci.spotified.ui.components.tapRow
import dev.darksavci.spotified.ui.theme.LocalPalette
import dev.darksavci.spotified.ui.theme.Type
import kotlinx.coroutines.CoroutineStart
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

/** A song in the list, under a key that stays with it as it moves. */
private class QueueRow(val key: String, val track: Track)

private fun rowsOf(queue: Queue): List<QueueRow> =
    queueKeys(queue.items.map { it.id }).zip(queue.items) { key, track -> QueueRow(key, track) }

private const val NEXT_HEADING = "heading:next"

/**
 * The queue: what has played, what plays now, and what is next. A song's handle
 * drags it, and the row follows the finger while the others make room; a long press
 * opens its menu, and a swipe sideways removes it, with an Undo. A song can only be
 * dropped below the one playing.
 *
 * The list shown is this page's own copy, changed at once by the finger and replaced
 * by the core's whenever that changes; if the core refuses something, nothing changes
 * and the copy goes back to the core's shortly after.
 */
@Composable
internal fun QueueSheet(actions: Actions) {
    val graph = actions.graph
    val projection by graph.playback.projection.collectAsStateWithLifecycle()
    val queue = projection.state.queue
    val latest by rememberUpdatedState(queue)
    val p = LocalPalette.current
    val haptics = rememberHaptics()
    val scope = rememberCoroutineScope()
    val density = LocalDensity.current
    val list = rememberLazyListState(initialFirstVisibleItemIndex = if (queue.index > 0) queue.index + 1 else 0)

    var rows by remember { mutableStateOf(rowsOf(queue)) }
    var dragging by remember { mutableStateOf<String?>(null) }
    // Where the dragged row's top is under the finger, from the top of the list.
    var fingerTop by remember { mutableFloatStateOf(0f) }
    var checking by remember { mutableStateOf<Job?>(null) }

    LaunchedEffect(queue.items, queue.index) {
        if (dragging == null) rows = rowsOf(queue)
    }
    val playingKey = queueKeys(queue.items.map { it.id }).getOrNull(queue.index)
    val playingAt = rows.indexOfFirst { it.key == playingKey }

    /** Gives the core a moment, then shows what it really has, in case it refused. */
    fun checkSoon() {
        checking?.cancel()
        checking = scope.launch {
            delay(2500)
            if (dragging == null) rows = rowsOf(latest)
        }
    }

    fun find(key: String): LazyListItemInfo? = list.layoutInfo.visibleItemsInfo.firstOrNull { it.key == key }

    /** Lets the dragged row pass whichever row its middle has come over. */
    fun passRows() {
        val key = dragging ?: return
        val dragged = find(key) ?: return
        val middle = fingerTop + dragged.size / 2f
        val over = list.layoutInfo.visibleItemsInfo.firstOrNull { item ->
            item.key != key && middle >= item.offset && middle < item.offset + item.size
        } ?: return
        val at = rows.indexOfFirst { it.key == key }
        val to = rows.indexOfFirst { it.key == over.key }
        val playing = rows.indexOfFirst { it.key == playingKey }
        if (at < 0 || to <= playing) return
        // A lazy list keeps its first row in place by key, which would carry the view along with a move of that row.
        if (dragged.index == list.firstVisibleItemIndex || over.index == list.firstVisibleItemIndex) {
            list.requestScrollToItem(list.firstVisibleItemIndex, list.firstVisibleItemScrollOffset)
        }
        rows = rows.moved(at, to)
        haptics.passed()
    }

    /** Keeps the dragged row in the list, and not above the heading of what is next. */
    fun moveFinger(by: Float) {
        val key = dragging ?: return
        val height = find(key)?.size ?: 0
        val info = list.layoutInfo
        val ceiling = find(NEXT_HEADING)?.let { it.offset + it.size }?.coerceAtLeast(info.viewportStartOffset) ?: info.viewportStartOffset
        val floor = (info.viewportEndOffset - height).coerceAtLeast(ceiling)
        fingerTop = (fingerTop + by).coerceIn(ceiling.toFloat(), floor.toFloat())
        passRows()
    }

    fun pickUp(key: String) {
        val row = find(key) ?: return
        checking?.cancel()
        fingerTop = row.offset.toFloat()
        dragging = key
        haptics.dragStart()
    }

    fun drop() {
        val key = dragging ?: return
        dragging = null
        haptics.drop()
        val live = latest
        val fresh = queueKeys(live.items.map { it.id })
        val to = dropPlace(fresh, rows.map { it.key }, key, live.index)
        if (to != null) {
            actions.playback.move(fresh.indexOf(key), to)
            checkSoon()
        } else {
            rows = rowsOf(live)
        }
    }

    fun remove(row: QueueRow) {
        val at = rows.indexOfFirst { it.key == row.key }
        if (at < 0) return
        rows = rows.filter { it.key != row.key }
        actions.playback.remove(at)
        actions.sayWithUndo("Removed ${row.track.title}") { actions.playback.send(Command.Enqueue(listOf(row.track), at)) }
        checkSoon()
    }

    /** The core clears what is next in one step, and keeps it for the Undo. */
    fun clear() {
        if (latest.items.size <= latest.index + 1) return
        rows = rows.take(playingAt + 1)
        actions.playback.send(Command.ClearUpcoming)
        actions.sayWithUndo("Queue cleared") { actions.playback.send(Command.RestoreUpcoming) }
        checkSoon()
    }

    // The list scrolls by itself while the row is held near either end of it.
    val zone = with(density) { 64.dp.toPx() }
    val fastest = with(density) { 14.dp.toPx() }
    LaunchedEffect(dragging != null) {
        if (dragging == null) return@LaunchedEffect
        while (true) {
            withFrameNanos { }
            val key = dragging ?: break
            val row = find(key) ?: continue
            val info = list.layoutInfo
            val speed = edgeScroll(fingerTop, fingerTop + row.size, info.viewportStartOffset.toFloat(), info.viewportEndOffset.toFloat(), zone, fastest)
            // Nothing above what is next can be dropped on, so the list does not go there.
            val atCeiling = speed < 0f && find(NEXT_HEADING).let { it != null && it.offset >= info.viewportStartOffset }
            if (speed != 0f && !atCeiling) {
                list.scrollBy(speed)
                passRows()
            }
        }
    }

    val played = rows.take(playingAt.coerceAtLeast(0))
    val current = rows.getOrNull(playingAt)
    val next = rows.drop(playingAt + 1)

    Row(Modifier.fillMaxWidth().padding(start = 16.dp, end = 8.dp, top = 8.dp, bottom = 4.dp), verticalAlignment = Alignment.CenterVertically) {
        Text("Queue", style = Type.heading, modifier = Modifier.weight(1f).semantics { heading() })
        // In a room, the queue is the room's.
        if (next.isNotEmpty() && !projection.followingRoom) {
            Text(
                "Clear queue", style = Type.label, color = p.secondary,
                modifier = Modifier.clip(RoundedCornerShape(16.dp)).tapRow(onClick = { clear() }, role = Role.Button).padding(horizontal = 12.dp, vertical = 10.dp),
            )
        }
    }
    LazyColumn(
        state = list,
        modifier = Modifier.heightIn(max = LocalConfiguration.current.screenHeightDp.dp * 0.68f),
    ) {
        if (played.isNotEmpty()) {
            item(key = "heading:played") { QueueHeading("Played", Modifier.animateItem()) }
            items(played, key = { it.key }) { row ->
                SwipeAway(row, ::remove, Modifier.animateItem(fadeOutSpec = null)) {
                    QueueSongRow(row.track, dim = true, onClick = { actions.playback.jump(rows.indexOf(row)) }, onMenu = { actions.menuFor(row.track, queueIndex = rows.indexOf(row)) })
                }
            }
        }
        if (current != null) {
            item(key = "heading:now") { QueueHeading("Now playing", Modifier.animateItem()) }
            item(key = current.key) {
                QueueSongRow(
                    current.track, playing = true, modifier = Modifier.animateItem(),
                    onClick = {}, onMenu = { actions.menuFor(current.track, queueIndex = playingAt) },
                )
            }
        }
        if (next.isNotEmpty()) {
            item(key = NEXT_HEADING) {
                QueueHeading("Next from: ${queue.origin.ifEmpty { "your queue" }}", Modifier.animateItem())
            }
            items(next, key = { it.key }) { row ->
                val held = dragging == row.key
                val place = if (held) {
                    Modifier.zIndex(1f).graphicsLayer {
                        translationY = fingerTop - (find(row.key)?.offset ?: fingerTop.toInt())
                        shadowElevation = 12.dp.toPx()
                    }.background(p.surfaceActive)
                } else Modifier.animateItem(fadeOutSpec = null)
                SwipeAway(row, ::remove, place, enabled = dragging == null) {
                    QueueSongRow(
                        row.track,
                        onClick = { actions.playback.jump(rows.indexOf(row)) },
                        onMenu = { actions.menuFor(row.track, queueIndex = rows.indexOf(row)) },
                        trailing = {
                            Box(
                                Modifier.size(48.dp)
                                    .pointerInput(row.key) {
                                        detectVerticalDragGestures(
                                            onDragStart = { pickUp(row.key) },
                                            onDragEnd = { drop() },
                                            onDragCancel = { drop() },
                                        ) { change, by ->
                                            change.consume()
                                            moveFinger(by)
                                        }
                                    }
                                    .semantics {
                                        stateDescription = "Drag to reorder"
                                        customActions = listOf(
                                            CustomAccessibilityAction("Move up") { nudge(actions, latest, row.key, -1) },
                                            CustomAccessibilityAction("Move down") { nudge(actions, latest, row.key, 1) },
                                        )
                                    },
                                contentAlignment = Alignment.Center,
                            ) {
                                Glyph(R.drawable.ic_grip_vertical, "Reorder ${row.track.title}", tint = p.secondary, size = 20.dp)
                            }
                        },
                    )
                }
            }
        }
    }
    Row(
        Modifier.fillMaxWidth().padding(16.dp).windowInsetsPadding(WindowInsets.navigationBars),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        val s = projection.state
        val timer by graph.sleep.endsAt.collectAsStateWithLifecycle()
        QueueButton(R.drawable.ic_shuffle, "Shuffle", s.shuffle, Modifier.weight(1f)) { actions.playback.setShuffle(!s.shuffle) }
        QueueButton(if (s.repeat == Repeat.One) R.drawable.ic_repeat_1 else R.drawable.ic_repeat, if (s.repeat == Repeat.One) "Repeat one" else "Repeat", s.repeat != Repeat.Off, Modifier.weight(1f)) { actions.playback.cycleRepeat() }
        QueueButton(R.drawable.ic_clock, "Timer", timer != null, Modifier.weight(1f)) { actions.menuFrom(MenuTarget.Queue, MenuTarget.SleepTimer) }
    }
}

/** A move for those who cannot drag: one place, staying below the song playing. */
private fun nudge(actions: Actions, queue: Queue, key: String, by: Int): Boolean {
    val at = queueKeys(queue.items.map { it.id }).indexOf(key)
    val to = at + by
    if (at < 0 || to <= queue.index || to > queue.items.lastIndex) return false
    actions.playback.move(at, to)
    return true
}

/** A row that comes off the list when swiped either way, showing what that will do behind it. */
@Composable
private fun SwipeAway(row: QueueRow, remove: (QueueRow) -> Unit, modifier: Modifier, enabled: Boolean = true, content: @Composable () -> Unit) {
    val p = LocalPalette.current
    val state = rememberSwipeToDismissBoxState(positionalThreshold = { it * 0.35f })
    val scope = rememberCoroutineScope()
    SwipeToDismissBox(
        state,
        backgroundContent = {
            Box(Modifier.fillMaxSize().background(p.danger).padding(horizontal = 20.dp), contentAlignment = Alignment.CenterEnd) {
                Glyph(R.drawable.ic_trash_2, "Remove", tint = p.text)
            }
        },
        modifier = modifier,
        gesturesEnabled = enabled,
        // The state is saved under the row's key, and asks for a removal whenever it
        // is composed swiped away. Put back by Undo, by a refused removal, or as the
        // next copy of the same song, the row must not come back swiped. Started
        // undispatched, for this row's scope ends as soon as the row has gone.
        onDismiss = {
            remove(row)
            scope.launch(start = CoroutineStart.UNDISPATCHED) { state.snapTo(SwipeToDismissBoxValue.Settled) }
        },
    ) {
        Box(Modifier.background(p.panel)) { content() }
    }
}

/**
 * A song in the queue. A tap ends if the finger moves, so a swipe across it never
 * plays it, and a long press opens its menu, which is all it does: only the handle drags.
 */
@Composable
private fun QueueSongRow(
    track: Track,
    onClick: () -> Unit,
    onMenu: () -> Unit,
    modifier: Modifier = Modifier,
    playing: Boolean = false,
    dim: Boolean = false,
    trailing: (@Composable () -> Unit)? = null,
) {
    val p = LocalPalette.current
    val alpha = if (dim || !track.playable) 0.55f else 1f
    Row(
        modifier.fillMaxWidth()
            .tapRow(onClick = onClick, onLongClick = onMenu, enabled = !playing, onClickLabel = "Play", onLongClickLabel = "More options")
            .semantics { if (playing) stateDescription = "Now playing" }
            .padding(start = 16.dp, end = 4.dp, top = 6.dp, bottom = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Art(track.artwork, 48.dp, Modifier.graphicsLayer { this.alpha = alpha })
        Spacer(Modifier.width(12.dp))
        Column(Modifier.weight(1f)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                if (playing) {
                    Glyph(R.drawable.ic_audio_lines, null, tint = p.accent, size = 16.dp)
                    Spacer(Modifier.width(6.dp))
                }
                Text(track.title, style = Type.body, maxLines = 1, overflow = TextOverflow.Ellipsis, color = (if (playing) p.accent else p.text).copy(alpha = alpha))
            }
            Text(track.artistNames, style = Type.small, maxLines = 1, overflow = TextOverflow.Ellipsis, color = p.secondary.copy(alpha = alpha))
        }
        trailing?.invoke()
        GlyphButton(R.drawable.ic_ellipsis, "More for ${track.title}", onMenu, tint = p.secondary, size = 20.dp)
    }
}

@Composable
private fun QueueHeading(text: String, modifier: Modifier = Modifier) {
    Text(
        text, style = Type.bodyStrong.copy(fontSize = 16.sp), maxLines = 1, overflow = TextOverflow.Ellipsis,
        modifier = modifier.fillMaxWidth().padding(start = 16.dp, end = 16.dp, top = 16.dp, bottom = 4.dp).semantics { heading() },
    )
}

@Composable
private fun QueueButton(icon: Int, label: String, on: Boolean, modifier: Modifier, onClick: () -> Unit) {
    val p = LocalPalette.current
    Column(
        modifier.clip(RoundedCornerShape(8.dp)).background(p.surface).tapRow(onClick = onClick, role = Role.Button).padding(vertical = 10.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Glyph(icon, null, tint = if (on) p.accent else p.text, size = 20.dp)
        Text(label, style = Type.tiny, color = if (on) p.accent else p.text)
    }
}
