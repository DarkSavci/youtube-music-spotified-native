package dev.darksavci.spotified.ui.player

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectHorizontalDragGestures
import androidx.compose.foundation.gestures.detectVerticalDragGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import dev.darksavci.spotified.R
import dev.darksavci.spotified.api.PlayState
import dev.darksavci.spotified.api.Repeat
import dev.darksavci.spotified.api.url
import dev.darksavci.spotified.data.Settings
import dev.darksavci.spotified.graph
import coil3.compose.AsyncImage
import dev.darksavci.spotified.ui.Actions
import dev.darksavci.spotified.ui.LocalActions
import dev.darksavci.spotified.ui.MenuTarget
import dev.darksavci.spotified.ui.components.Art
import dev.darksavci.spotified.ui.components.ExplicitBadge
import dev.darksavci.spotified.ui.components.Glyph
import dev.darksavci.spotified.ui.components.GlyphButton
import dev.darksavci.spotified.ui.components.artColor
import dev.darksavci.spotified.ui.components.darkened
import dev.darksavci.spotified.ui.theme.LocalPalette
import dev.darksavci.spotified.ui.theme.Type
import kotlinx.coroutines.delay

/** Now Playing's styles, as Settings names them. */
const val STYLE_FULL_ART = 1
const val STYLE_LYRICS_FIRST = 2

/** Where the song is, kept moving between the core's reports. */
@Composable
fun rememberPosition(): Pair<Long, Long> {
    val graph = LocalContext.current.graph
    val projection by graph.playback.projection.collectAsStateWithLifecycle()
    var position by remember { mutableLongStateOf(0L) }
    var duration by remember { mutableLongStateOf(0L) }
    LaunchedEffect(projection) {
        while (true) {
            val current = projection.state.current
            val engine = graph.playback.engine
            if (engine != null && current != null && engine.currentId == current.id) {
                position = engine.positionMs
                duration = engine.durationMs.takeIf { it > 0 } ?: current.durationMs
            } else {
                position = projection.state.positionMs
                duration = current?.durationMs ?: 0
            }
            delay(250)
        }
    }
    return position to duration
}

fun clock(ms: Long): String {
    val s = (ms / 1000).coerceAtLeast(0)
    return if (s >= 3600) "%d:%02d:%02d".format(s / 3600, s / 60 % 60, s % 60) else "%d:%02d".format(s / 60, s % 60)
}

/** The card above the bottom bar: art, title, devices and play, with a thin progress line. */
@Composable
fun MiniPlayer(onOpen: () -> Unit) {
    val graph = LocalContext.current.graph
    val projection by graph.playback.projection.collectAsStateWithLifecycle()
    val track = projection.state.current ?: return
    val p = LocalPalette.current
    val tint = artColor(track.artwork).darkened(0.5f)
    val (position, duration) = rememberPosition()
    val playing = projection.target.playing
    Column(
        Modifier
            .padding(horizontal = 8.dp)
            .clip(RoundedCornerShape(8.dp))
            .background(tint)
            .clickable(onClickLabel = "Open the player", onClick = onOpen)
            .pointerInput(Unit) {
                detectVerticalDragGestures { _, drag -> if (drag < -12) onOpen() }
            }
            .pointerInput(track.id) {
                var total = 0f
                detectHorizontalDragGestures(
                    onDragEnd = {
                        if (total < -120) graph.playback.next() else if (total > 120) graph.playback.previous()
                        total = 0f
                    },
                ) { _, drag -> total += drag }
            },
    ) {
        Row(Modifier.padding(start = 8.dp, top = 8.dp, bottom = 6.dp, end = 4.dp), verticalAlignment = Alignment.CenterVertically) {
            Art(track.artwork, 40.dp)
            Spacer(Modifier.width(10.dp))
            Column(Modifier.weight(1f)) {
                Text(track.title, style = Type.label, maxLines = 1, overflow = TextOverflow.Ellipsis, color = Color.White)
                Text(track.artistNames, style = Type.small, maxLines = 1, overflow = TextOverflow.Ellipsis, color = Color.White.copy(alpha = 0.7f))
            }
            val liked by graph.liked.ids.collectAsStateWithLifecycle()
            GlyphButton(
                if (track.id in liked) R.drawable.ic_heart_filled else R.drawable.ic_heart,
                if (track.id in liked) "Remove from Liked" else "Like",
                { graph.liked.toggle(track) },
                tint = if (track.id in liked) p.accent else Color.White,
            )
            GlyphButton(
                if (playing) R.drawable.ic_pause_filled else R.drawable.ic_play_filled,
                if (playing) "Pause" else "Play",
                graph.playback::toggle,
                tint = Color.White,
            )
        }
        val fraction = if (duration > 0) (position.toFloat() / duration).coerceIn(0f, 1f) else 0f
        Box(Modifier.fillMaxWidth().padding(horizontal = 8.dp).height(2.dp).clip(RoundedCornerShape(1.dp)).background(Color.White.copy(alpha = 0.25f))) {
            Box(Modifier.fillMaxWidth(fraction).height(2.dp).background(Color.White))
        }
        Spacer(Modifier.height(2.dp))
    }
}

/** The full-screen player, over a gradient from the art's colour. */
@Composable
fun NowPlaying(onClose: () -> Unit) {
    val actions = LocalActions.current
    val graph = actions.graph
    val projection by graph.playback.projection.collectAsStateWithLifecycle()
    val p = LocalPalette.current
    val state = projection.state
    val track = state.current
    val tint = artColor(track?.artwork.orEmpty())
    val liked by graph.liked.ids.collectAsStateWithLifecycle()
    val settings by graph.settings.settings.collectAsStateWithLifecycle(Settings())
    val style = settings.nowPlayingStyle

    if (actions.lyricsOpen && track != null) {
        LyricsScreen(track, tint, onClose = { actions.lyricsOpen = false })
        return
    }

    Box(
        Modifier.fillMaxSize()
            .background(Brush.verticalGradient(listOf(tint.darkened(0.25f), tint.darkened(0.7f), p.window)))
            .clickable(enabled = false) {}
            .pointerInput(Unit) { detectVerticalDragGestures { _, drag -> if (drag > 30) onClose() } },
    ) {
        // The full-art style: the picture is the page, darkened under the controls.
        if (style == STYLE_FULL_ART && track != null) {
            AsyncImage(track.artwork.url(1080), null, Modifier.fillMaxSize(), contentScale = ContentScale.Crop)
            Box(Modifier.fillMaxSize().background(Brush.verticalGradient(listOf(Color.Black.copy(alpha = 0.35f), Color.Transparent, Color.Black.copy(alpha = 0.85f)))))
        }
        Column(
            Modifier.fillMaxSize().windowInsetsPadding(WindowInsets.safeDrawing).verticalScroll(rememberScrollState()),
        ) {
            Row(Modifier.fillMaxWidth().padding(horizontal = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                GlyphButton(R.drawable.ic_chevron_down, "Close the player", onClose)
                Column(Modifier.weight(1f), horizontalAlignment = Alignment.CenterHorizontally) {
                    Text(if (projection.followingRoom) "LISTENING TOGETHER" else "PLAYING FROM", style = Type.tiny, color = Color.White.copy(alpha = 0.7f))
                    Text(state.queue.origin.ifEmpty { "Your queue" }, style = Type.label, maxLines = 1, overflow = TextOverflow.Ellipsis, color = Color.White)
                }
                GlyphButton(R.drawable.ic_ellipsis, "More", { track?.let { actions.menuFor(it) } })
            }
            if (track == null) {
                Box(Modifier.fillMaxWidth().padding(64.dp), contentAlignment = Alignment.Center) {
                    Text("Nothing is playing", style = Type.body, color = p.secondary)
                }
                return@Column
            }
            Spacer(Modifier.height(24.dp))
            BoxWithConstraints(Modifier.fillMaxWidth().padding(horizontal = 24.dp)) {
            when (style) {
                // The art is the background already; the space it took stays, for the swipe.
                STYLE_FULL_ART -> Spacer(Modifier.size(maxWidth))
                STYLE_LYRICS_FIRST -> LyricsPanel(track, maxWidth, tint, onExpand = { actions.lyricsOpen = true })
                else -> Art(
                track.artwork,
                size = maxWidth,
                modifier = Modifier
                    .pointerInput(track.id) {
                        var total = 0f
                        detectHorizontalDragGestures(
                            onDragEnd = {
                                if (total < -120) graph.playback.next() else if (total > 120) graph.playback.previous()
                                total = 0f
                            },
                        ) { _, drag -> total += drag }
                    },
                shape = RoundedCornerShape(8.dp),
            )
            }
            }
            Spacer(Modifier.height(28.dp))
            Row(Modifier.padding(horizontal = 24.dp), verticalAlignment = Alignment.CenterVertically) {
                Column(Modifier.weight(1f)) {
                    Text(
                        track.title, style = Type.title, color = Color.White, maxLines = 2, overflow = TextOverflow.Ellipsis,
                        modifier = Modifier.clickable { actions.openAlbumOf(track) },
                    )
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        if (track.explicit) { ExplicitBadge(); Spacer(Modifier.width(6.dp)) }
                        Text(
                            track.artistNames, style = Type.body, color = Color.White.copy(alpha = 0.7f), maxLines = 1,
                            overflow = TextOverflow.Ellipsis, modifier = Modifier.clickable { actions.openArtistOf(track) },
                        )
                    }
                }
                val isLiked = track.id in liked
                GlyphButton(
                    if (isLiked) R.drawable.ic_heart_filled else R.drawable.ic_heart,
                    if (isLiked) "Remove from Liked" else "Like",
                    { graph.liked.toggle(track) },
                    tint = if (isLiked) p.accent else Color.White, size = 28.dp,
                )
            }
            Spacer(Modifier.height(16.dp))
            Seekbar()
            Controls(actions, projection.target.playing, state.shuffle, state.repeat, state.state)
            Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp), verticalAlignment = Alignment.CenterVertically) {
                GlyphButton(R.drawable.ic_clock, "Sleep timer", { actions.menu = MenuTarget.SleepTimer }, tint = sleepTint())
                GlyphButton(R.drawable.ic_sliders_vertical, "Speed", { actions.menu = MenuTarget.Speed })
                Spacer(Modifier.weight(1f))
                GlyphButton(R.drawable.ic_share_2, "Share", { actions.share(track.title, Actions.songLink(track.id)) })
                GlyphButton(R.drawable.ic_list_music, "Queue", { actions.menu = MenuTarget.Queue })
            }
            Spacer(Modifier.height(16.dp))
            if (style != STYLE_LYRICS_FIRST) LyricsCard(track, tint, onExpand = { actions.lyricsOpen = true })
            Spacer(Modifier.height(32.dp))
        }
    }
}

@Composable
private fun sleepTint(): Color {
    val timer by LocalContext.current.graph.sleep.endsAt.collectAsStateWithLifecycle()
    return if (timer != null) LocalPalette.current.accent else Color.White
}

@Composable
private fun Seekbar() {
    val graph = LocalContext.current.graph
    val (position, duration) = rememberPosition()
    var dragging by remember { mutableStateOf(false) }
    var dragValue by remember { mutableFloatStateOf(0f) }
    val shown = if (dragging) dragValue else if (duration > 0) position.toFloat() / duration else 0f
    Column(Modifier.padding(horizontal = 24.dp)) {
        ThinSeekBar(
            fraction = shown.coerceIn(0f, 1f),
            onDrag = { dragging = true; dragValue = it },
            onRelease = {
                dragging = false
                if (duration > 0) graph.playback.seek((dragValue * duration).toLong())
            },
        )
        val at = if (dragging) (dragValue * duration).toLong() else position
        Row(Modifier.fillMaxWidth()) {
            Text(clock(at), style = Type.tiny, color = Color.White.copy(alpha = 0.7f))
            Spacer(Modifier.weight(1f))
            // Spotify counts down what is left.
            Text("-" + clock((duration - at).coerceAtLeast(0)), style = Type.tiny, color = Color.White.copy(alpha = 0.7f))
        }
    }
}

/**
 * Spotify's seek bar: a thin line with a small dot that grows while it is
 * held. A tap or a drag anywhere along it moves to that point.
 */
@Composable
private fun ThinSeekBar(fraction: Float, onDrag: (Float) -> Unit, onRelease: () -> Unit) {
    var held by remember { mutableStateOf(false) }
    val dot by animateFloatAsState(if (held) 8f else 6f, label = "dot")
    Canvas(
        Modifier.fillMaxWidth().height(28.dp)
            .pointerInput(Unit) {
                detectTapGestures(
                    onPress = { o ->
                        held = true
                        onDrag((o.x / size.width).coerceIn(0f, 1f))
                        tryAwaitRelease()
                        held = false
                        onRelease()
                    },
                )
            }
            .pointerInput(Unit) {
                detectHorizontalDragGestures(
                    onDragStart = { held = true },
                    onDragEnd = { held = false; onRelease() },
                    onDragCancel = { held = false; onRelease() },
                ) { change, _ -> onDrag((change.position.x / size.width).coerceIn(0f, 1f)) }
            },
    ) {
        val y = size.height / 2
        val stroke = 4.dp.toPx()
        drawLine(Color.White.copy(alpha = 0.3f), Offset(0f, y), Offset(size.width, y), stroke, StrokeCap.Round)
        val x = size.width * fraction
        drawLine(Color.White, Offset(0f, y), Offset(x, y), stroke, StrokeCap.Round)
        drawCircle(Color.White, dot.dp.toPx(), Offset(x, y))
    }
}

@Composable
private fun Controls(actions: Actions, playing: Boolean, shuffle: Boolean, repeat: Repeat, state: PlayState) {
    val p = LocalPalette.current
    val playback = actions.playback
    Row(
        Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        GlyphButton(R.drawable.ic_shuffle, if (shuffle) "Shuffle on" else "Shuffle off", { playback.setShuffle(!shuffle) }, tint = if (shuffle) p.accent else Color.White)
        GlyphButton(R.drawable.ic_skip_back_filled, "Previous", playback::previous, tint = Color.White, size = 32.dp)
        val scale by animateFloatAsState(if (state == PlayState.Loading || state == PlayState.Stalled) 0.94f else 1f, label = "play")
        Box(
            Modifier.size(64.dp * scale).clip(CircleShape).background(Color.White)
                .clickable(onClickLabel = if (playing) "Pause" else "Play", role = Role.Button, onClick = playback::toggle),
            contentAlignment = Alignment.Center,
        ) {
            Glyph(if (playing) R.drawable.ic_pause_filled else R.drawable.ic_play_filled, if (playing) "Pause" else "Play", tint = Color.Black, size = 28.dp)
        }
        GlyphButton(R.drawable.ic_skip_forward_filled, "Next", playback::next, tint = Color.White, size = 32.dp)
        GlyphButton(
            if (repeat == Repeat.One) R.drawable.ic_repeat_1 else R.drawable.ic_repeat,
            when (repeat) { Repeat.Off -> "Repeat off"; Repeat.All -> "Repeat all"; Repeat.One -> "Repeat one" },
            playback::cycleRepeat,
            tint = if (repeat == Repeat.Off) Color.White else p.accent,
        )
    }
}
