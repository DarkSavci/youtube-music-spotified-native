package dev.darksavci.spotified.ui.player

import dev.darksavci.spotified.api.Projection
import androidx.compose.ui.platform.LocalLayoutDirection
import androidx.compose.foundation.layout.calculateStartPadding
import androidx.compose.foundation.layout.calculateEndPadding
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.union
import androidx.compose.foundation.layout.displayCutout
import android.content.res.Configuration
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.Orientation
import androidx.compose.foundation.gestures.anchoredDraggable
import androidx.compose.foundation.gestures.detectHorizontalDragGestures
import androidx.compose.foundation.interaction.DragInteraction
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.asPaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.systemBars
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.windowInsetsBottomHeight
import androidx.compose.foundation.layout.windowInsetsTopHeight
import androidx.compose.foundation.ScrollState
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.pager.PagerState
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.isSpecified
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import coil3.compose.AsyncImage
import dev.darksavci.spotified.R
import dev.darksavci.spotified.api.PlayState
import dev.darksavci.spotified.api.Artwork
import dev.darksavci.spotified.api.Queue
import dev.darksavci.spotified.api.Repeat
import dev.darksavci.spotified.api.Track
import dev.darksavci.spotified.api.url
import dev.darksavci.spotified.data.Settings
import dev.darksavci.spotified.graph
import dev.darksavci.spotified.ui.Actions
import dev.darksavci.spotified.ui.LocalActions
import dev.darksavci.spotified.ui.MenuTarget
import dev.darksavci.spotified.ui.Shared
import dev.darksavci.spotified.ui.components.Art
import dev.darksavci.spotified.ui.components.ExplicitBadge
import dev.darksavci.spotified.ui.components.Glyph
import dev.darksavci.spotified.ui.components.GlyphButton
import dev.darksavci.spotified.ui.components.artColor
import dev.darksavci.spotified.ui.components.darkened
import dev.darksavci.spotified.ui.components.scrub
import dev.darksavci.spotified.ui.components.scrubSemantics
import dev.darksavci.spotified.ui.components.tapRow
import dev.darksavci.spotified.ui.theme.DarkPalette
import dev.darksavci.spotified.ui.theme.LocalPalette
import dev.darksavci.spotified.ui.theme.Type
import java.util.Locale
import kotlin.math.abs
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.flow.filter
import kotlinx.coroutines.launch

/** Now Playing's styles, as Settings names them. */
const val STYLE_FULL_ART = 1
const val STYLE_LYRICS_FIRST = 2

/** Where the song is, kept moving between the core's reports. */
@Composable
fun rememberPosition(periodMs: Long = 250): Pair<Long, Long> {
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
            delay(periodMs)
        }
    }
    return position to duration
}

fun clock(ms: Long): String {
    val s = (ms / 1000).coerceAtLeast(0)
    return if (s >= 3600) "%d:%02d:%02d".format(Locale.US, s / 3600, s / 60 % 60, s % 60) else "%d:%02d".format(Locale.US, s / 60, s % 60)
}

/** The player's play button is a spinner while the song is on its way, as there is nothing to hear yet. */
private fun PlayState.waiting() = this == PlayState.Loading || this == PlayState.Stalled

/**
 * The card above the bottom bar: art, title, where it plays, like and play, with a
 * thin progress line. It opens the player by a tap or by being pushed up, and the
 * player rises out of it, following the finger, as it fades.
 */
@Composable
internal fun MiniPlayer(sheet: DragSheet, onOpen: () -> Unit, modifier: Modifier = Modifier) {
    val graph = LocalContext.current.graph
    val projection by graph.playback.projection.collectAsStateWithLifecycle()
    val track = projection.state.current ?: return
    val p = LocalPalette.current
    val tint = rememberTint(track.artwork).darkened(0.5f)
    val (position, duration) = rememberPosition()
    val playing = projection.target.playing
    val waiting = projection.state.state.waiting()
    Column(
        modifier
            .padding(horizontal = 8.dp)
            .graphicsLayer { alpha = 1f - (sheet.openness * FADE_OVER).coerceIn(0f, 1f) }
            .clip(RoundedCornerShape(8.dp))
            .background(tint)
            .dragsSheet(sheet)
            .anchoredDraggable(sheet.drag, Orientation.Vertical, flingBehavior = sheetFling(sheet))
            .tapRow(onClick = onOpen, onClickLabel = "Open the player")
            .pointerInput(track.id) {
                var total = 0f
                detectHorizontalDragGestures(
                    onDragEnd = {
                        // Back is always the song before: the core's previous only restarts one well under way.
                        val at = graph.playback.projection.value.state.queue.index
                        when {
                            total < -120 -> graph.playback.next()
                            total > 120 -> if (at > 0) graph.playback.jump(at - 1) else graph.playback.previous()
                        }
                        total = 0f
                    },
                ) { _, drag -> total += drag }
            },
    ) {
        Row(Modifier.padding(start = 8.dp, top = 8.dp, bottom = 6.dp, end = 4.dp), verticalAlignment = Alignment.CenterVertically) {
            SteadyArt(track.artwork, 40.dp)
            Spacer(Modifier.width(10.dp))
            Column(Modifier.weight(1f)) {
                Text(track.title, style = Type.label, maxLines = 1, overflow = TextOverflow.Ellipsis, color = Color.White)
                Text(track.artistNames, style = Type.small, maxLines = 1, overflow = TextOverflow.Ellipsis, color = Color.White.copy(alpha = 0.7f))
            }
            OutputButton()
            val liked by graph.liked.ids.collectAsStateWithLifecycle()
            // A swipe can start on these, so they answer only to a tap and do not light up under a passing finger.
            val isLiked = track.id in liked
            Box(
                Modifier.size(48.dp).clip(CircleShape)
                    .tapRow(onClick = { graph.liked.toggle(track) }, role = Role.Button, onClickLabel = if (isLiked) "Remove from Liked" else "Like"),
                contentAlignment = Alignment.Center,
            ) {
                Glyph(
                    if (isLiked) R.drawable.ic_heart_filled else R.drawable.ic_heart,
                    if (isLiked) "Remove from Liked" else "Like",
                    tint = if (isLiked) p.accent else Color.White,
                )
            }
            Box(
                Modifier.size(48.dp).clip(CircleShape)
                    .tapRow(onClick = graph.playback::toggle, role = Role.Button, onClickLabel = if (playing) "Pause" else "Play")
                    .semantics { if (waiting) stateDescription = "Loading" },
                contentAlignment = Alignment.Center,
            ) {
                PlayGlyph(playing, waiting, Color.White, 24.dp)
            }
        }
        val fraction = if (duration > 0) (position.toFloat() / duration).coerceIn(0f, 1f) else 0f
        Box(Modifier.fillMaxWidth().padding(horizontal = 8.dp).height(2.dp).clip(RoundedCornerShape(1.dp)).background(Color.White.copy(alpha = 0.25f))) {
            Box(Modifier.fillMaxWidth(fraction).height(2.dp).background(Color.White))
        }
        Spacer(Modifier.height(2.dp))
    }
}

/**
 * The colour of a song's art, darkened so that white reads on it, moving to the next
 * song's rather than snapping; it holds the last one while the next cover is read.
 */
@Composable
private fun rememberTint(artwork: List<Artwork>): Color {
    val fallback = LocalPalette.current.surfaceActive
    val held = remember { arrayOf(fallback) }
    val found = artColor(artwork, Color.Unspecified)
    if (found.isSpecified) held[0] = found
    val tint by animateColorAsState(readableBehindWhite(held[0]), tween(400), label = "tint")
    return tint
}

/** A cover that stays on show until the next one has been read, rather than going blank between songs. */
@Composable
private fun SteadyArt(artwork: List<Artwork>, size: Dp) {
    val address = artwork.url(with(LocalDensity.current) { size.roundToPx() }.coerceAtLeast(60))
    var last by remember { mutableStateOf<String?>(null) }
    Box(Modifier.size(size).clip(RoundedCornerShape(4.dp)).background(LocalPalette.current.surface)) {
        if (last != null && last != address) AsyncImage(last, null, Modifier.fillMaxSize(), contentScale = ContentScale.Crop)
        AsyncImage(address, null, Modifier.fillMaxSize(), contentScale = ContentScale.Crop, onSuccess = { last = address })
    }
}

/** What a play button shows: play, pause, or a spinner while the song loads or has stalled. */
@Composable
internal fun PlayGlyph(playing: Boolean, waiting: Boolean, tint: Color, size: Dp) {
    if (waiting) {
        CircularProgressIndicator(Modifier.size(size * 0.85f), color = tint, strokeWidth = 2.5.dp)
    } else {
        Glyph(if (playing) R.drawable.ic_pause_filled else R.drawable.ic_play_filled, if (playing) "Pause" else "Play", tint = tint, size = size)
    }
}

/**
 * The full-screen player, over a gradient from the art's colour. It is dark
 * whatever the app's theme, as Spotify's is: white controls on the art's colour.
 */
@Composable
fun NowPlaying(onClose: () -> Unit) {
    CompositionLocalProvider(LocalPalette provides DarkPalette) { NowPlayingPage(onClose) }
}

/**
 * The covers' pager, kept in step with the queue both ways. A swipe that lands on
 * another song asks for it; if nothing comes of that, the pager goes back to what
 * is really playing. The queue moving by itself turns the page, unless a finger is on it.
 */
@Composable
private fun rememberQueuePager(queue: Queue, choose: (Int) -> Unit): PagerState {
    val pager = rememberPagerState(initialPage = queue.index) { queue.items.size }
    val latest by rememberUpdatedState(queue)
    val chosen by rememberUpdatedState(choose)
    LaunchedEffect(queue.index) {
        if (pager.currentPage != queue.index && !pager.isScrollInProgress) {
            if (abs(pager.currentPage - queue.index) == 1) pager.animateScrollToPage(queue.index)
            else pager.scrollToPage(queue.index)
        }
    }
    // Only a finger chooses a song: the pager also moves by itself when the queue changes under it.
    var swiped by remember { mutableStateOf(false) }
    LaunchedEffect(pager) {
        pager.interactionSource.interactions.collect { if (it is DragInteraction.Start) swiped = true }
    }
    LaunchedEffect(pager) {
        snapshotFlow { pager.isScrollInProgress }.filter { !it }.collectLatest {
            if (!swiped) return@collectLatest
            swiped = false
            val page = pager.currentPage
            if (page == latest.index || page !in latest.items.indices) return@collectLatest
            chosen(page)
            delay(2000)
            if (pager.currentPage != latest.index && !pager.isScrollInProgress) pager.animateScrollToPage(latest.index)
        }
    }
    return pager
}

@Composable
private fun NowPlayingPage(onClose: () -> Unit) {
    val actions = LocalActions.current
    val graph = actions.graph
    val projection by graph.playback.projection.collectAsStateWithLifecycle()
    val p = LocalPalette.current
    val state = projection.state
    val track = state.current
    val queue = state.queue
    val tint = rememberTint(track?.artwork.orEmpty())
    val liked by graph.liked.ids.collectAsStateWithLifecycle()
    val settings by graph.settings.settings.collectAsStateWithLifecycle(Settings())
    val style = settings.nowPlayingStyle
    val pager = rememberQueuePager(queue) { graph.playback.jump(it) }
    val keys = remember(queue.items) { queueKeys(queue.items.map { it.id }) }
    val scroll = rememberScrollState()
    val wide = LocalConfiguration.current.orientation == Configuration.ORIENTATION_LANDSCAPE
    val fullArt = style == STYLE_FULL_ART && track != null && !wide

    Box(
        Modifier.fillMaxSize()
            .background(
                if (fullArt) Brush.verticalGradient(listOf(tint.darkened(0.7f), p.window))
                else Brush.verticalGradient(listOf(tint.darkened(0.25f), tint.darkened(0.7f), p.window)),
            )
            // Nothing under the player is touched through it.
            .pointerInput(Unit) {},
    ) {
        if (fullArt) FullArtCover(queue, keys, pager, scroll, tint.darkened(0.7f))
        if (wide && track != null) {
            // On its side, the cover has the height and the rest scrolls beside it.
            val bars = WindowInsets.systemBars.union(WindowInsets.displayCutout).asPaddingValues()
            Row(Modifier.fillMaxSize()) {
                BoxWithConstraints(Modifier.fillMaxHeight().weight(0.42f).padding(bars.onlyStart()), contentAlignment = Alignment.Center) {
                    val side = minOf(maxWidth, maxHeight) - 24.dp
                    ArtPager(queue, keys, pager, if (style == STYLE_FULL_ART) 0 else style, tint, side = side, onLyrics = { actions.lyricsOpen = true })
                }
                Column(Modifier.fillMaxHeight().weight(0.58f).verticalScroll(scroll).padding(bars.onlyEnd())) {
                    PlayerTopBar(projection.followingRoom, state.queue.origin, track, actions, onClose)
                    Spacer(Modifier.height(4.dp))
                    PlayerBody(track, projection, settings, tint, actions)
                }
            }
        } else {
        // The bars' room is inside the scroll, so the page passes under them rather than being cut off at them.
        Column(
            Modifier.fillMaxSize().verticalScroll(scroll).padding(WindowInsets.systemBars.asPaddingValues()),
        ) {
            PlayerTopBar(projection.followingRoom, state.queue.origin, track, actions, onClose)
            if (track == null) {
                Box(Modifier.fillMaxWidth().padding(64.dp), contentAlignment = Alignment.Center) {
                    Text("Nothing is playing", style = Type.body, color = p.secondary)
                }
                return@Column
            }
            Spacer(Modifier.height(24.dp))
            ArtPager(queue, keys, pager, style, tint, onLyrics = { actions.lyricsOpen = true })
            Spacer(Modifier.height(28.dp))
            PlayerBody(track, projection, settings, tint, actions)
        }
        }
        // Under the system's bars, the page fades out rather than ending at a hard edge. At the top of the page there is nothing to fade.
        val top = if (fullArt) Color.Black else tint.darkened(0.25f)
        Column(Modifier.fillMaxWidth().graphicsLayer { alpha = (scroll.value / 120f).coerceIn(0f, 1f) }) {
            Box(Modifier.fillMaxWidth().windowInsetsTopHeight(WindowInsets.statusBars).background(top.copy(alpha = 0.75f)))
            Box(Modifier.fillMaxWidth().height(20.dp).background(Brush.verticalGradient(listOf(top.copy(alpha = 0.75f), Color.Transparent))))
        }
        Column(Modifier.fillMaxWidth().align(Alignment.BottomCenter)) {
            Box(Modifier.fillMaxWidth().height(20.dp).background(Brush.verticalGradient(listOf(Color.Transparent, p.window.copy(alpha = 0.85f)))))
            Box(Modifier.fillMaxWidth().windowInsetsBottomHeight(WindowInsets.navigationBars).background(p.window.copy(alpha = 0.85f)))
        }
        if (track != null) {
            val lyrics = rememberDragSheet(if (actions.lyricsOpen) SheetAt.Open else SheetAt.Closed)
            SheetLayer(lyrics, actions.lyricsOpen, { actions.lyricsOpen = it }) {
                LyricsScreen(track, tint, onClose = { actions.lyricsOpen = false })
            }
        }
    }
}

/** The player's top line: the way down, where it plays from, and its menu. */
@Composable
private fun PlayerTopBar(followingRoom: Boolean, origin: String, track: Track?, actions: Actions, onClose: () -> Unit) {
    Row(Modifier.fillMaxWidth().padding(horizontal = 8.dp), verticalAlignment = Alignment.CenterVertically) {
        GlyphButton(R.drawable.ic_chevron_down, "Close the player", onClose)
        Column(Modifier.weight(1f), horizontalAlignment = Alignment.CenterHorizontally) {
            Text(if (followingRoom) "LISTENING TOGETHER" else "PLAYING FROM", style = Type.tiny, color = Color.White.copy(alpha = 0.7f))
            Text(origin.ifEmpty { "Your queue" }, style = Type.label, maxLines = 1, overflow = TextOverflow.Ellipsis, color = Color.White)
        }
        GlyphButton(R.drawable.ic_ellipsis, "More", { track?.let { actions.menuFor(it) } })
    }
}

/** Everything of the player below its cover: the song, the seek bar, the controls and the cards. */
@Composable
private fun PlayerBody(track: Track, projection: Projection, settings: Settings, tint: Color, actions: Actions) {
    val graph = actions.graph
    val p = LocalPalette.current
    val state = projection.state
    val liked by graph.liked.ids.collectAsStateWithLifecycle()
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
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.clickable {
                        // A song by several: each is as much its artist as the first.
                        if (track.artists.count { it.id.isNotEmpty() } > 1) actions.menu = MenuTarget.Artists(track)
                        else actions.openArtistOf(track)
                    },
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
    if (settings.visualizer) {
        SpectrumBars(graph.playback.engine, projection.target.playing, Color.White.copy(alpha = 0.7f), Modifier.padding(horizontal = 24.dp).fillMaxWidth().height(40.dp))
        Spacer(Modifier.height(8.dp))
    }
    Seekbar(Modifier.padding(horizontal = 24.dp), track.id)
    Controls(actions, projection.target.playing, state.shuffle, state.repeat, state.state)
    Row(Modifier.fillMaxWidth().padding(horizontal = 8.dp), verticalAlignment = Alignment.CenterVertically) {
        OutputButton(named = true)
        GlyphButton(R.drawable.ic_clock, "Sleep timer", { actions.menu = MenuTarget.SleepTimer }, tint = sleepTint())
        SpeedButton(settings.speed) { actions.menu = MenuTarget.Speed }
        Spacer(Modifier.weight(1f))
        GlyphButton(R.drawable.ic_share_2, "Share", { actions.menu = MenuTarget.Share(Shared.Song(track)) })
        GlyphButton(R.drawable.ic_list_music, "Queue", { actions.menu = MenuTarget.Queue })
    }
    Spacer(Modifier.height(16.dp))
    if (settings.nowPlayingStyle != STYLE_LYRICS_FIRST) LyricsCard(track, tint, onExpand = { actions.lyricsOpen = true })
    PlayerCards(track, actions)
    Spacer(Modifier.height(32.dp))
}

/** What of these insets the start of the screen needs; the top and bottom too, as a column beside it has them. */
@Composable
private fun PaddingValues.onlyStart(): PaddingValues {
    val dir = LocalLayoutDirection.current
    return PaddingValues(start = calculateStartPadding(dir), top = calculateTopPadding(), bottom = calculateBottomPadding())
}

@Composable
private fun PaddingValues.onlyEnd(): PaddingValues {
    val dir = LocalLayoutDirection.current
    return PaddingValues(end = calculateEndPadding(dir), top = calculateTopPadding(), bottom = calculateBottomPadding())
}

/**
 * The covers in a row, one to a song in the queue, following the finger, with the
 * edges of the next and the last showing. In the full-art style the picture is behind
 * the page, so this is an empty space that moves it; in the lyrics-first style the
 * song playing shows its words, and those beside it their covers.
 */
@Composable
private fun ArtPager(queue: Queue, keys: List<String>, pager: PagerState, style: Int, tint: Color, side: Dp? = null, onLyrics: () -> Unit) {
    BoxWithConstraints(Modifier.fillMaxWidth()) {
        // Not taller than most of the screen.
        val side = side ?: minOf(maxWidth - 48.dp, LocalConfiguration.current.screenHeightDp.dp * 0.6f)
        HorizontalPager(
            pager,
            contentPadding = PaddingValues(horizontal = (maxWidth - side) / 2),
            pageSpacing = 12.dp,
            beyondViewportPageCount = 1,
            key = { keys.getOrElse(it) { "gone$it" } },
        ) { page ->
            val shown = queue.items.getOrNull(page) ?: return@HorizontalPager
            when {
                style == STYLE_FULL_ART -> Spacer(Modifier.size(side))
                style == STYLE_LYRICS_FIRST && page == queue.index -> LyricsPanel(shown, side, tint, onExpand = onLyrics)
                else -> Art(shown.artwork, side, shape = RoundedCornerShape(8.dp), description = "Cover of ${shown.title}")
            }
        }
    }
}

/**
 * The full-art style's picture: the whole cover, edge to edge and from the very top,
 * fading into the page at its foot so that the words below stay on the page's own
 * colour. It moves sideways with the pager, showing those beside it, and scrolls away
 * with the page, so no words ever run over it. It is asked for at the width it is drawn.
 */
@Composable
private fun FullArtCover(queue: Queue, keys: List<String>, pager: PagerState, scroll: ScrollState, page: Color) {
    BoxWithConstraints(
        Modifier.fillMaxWidth().graphicsLayer {
            translationY = -scroll.value.toFloat()
            clip = true
        },
    ) {
        val side = minOf(maxWidth, LocalConfiguration.current.screenHeightDp.dp)
        val px = with(LocalDensity.current) { maxWidth.roundToPx() }.coerceAtLeast(1080)
        Box(Modifier.fillMaxWidth().height(side)) {
            val centre = pager.currentPage
            for (at in centre - 1..centre + 1) {
                val shown: Track = queue.items.getOrNull(at) ?: continue
                key(keys.getOrElse(at) { "gone$at" }) {
                    AsyncImage(
                        shown.artwork.url(px), "Cover of ${shown.title}",
                        Modifier.fillMaxSize().graphicsLayer {
                            translationX = (at - pager.currentPage - pager.currentPageOffsetFraction) * size.width
                        },
                        contentScale = ContentScale.Crop,
                    )
                }
            }
            Box(Modifier.fillMaxSize().background(Brush.verticalGradient(0.8f to Color.Transparent, 1f to page)))
            // Behind the top bar, measured from it rather than from the cover: at any
            // cover's size, and on a white one, its words and buttons read.
            val bar = WindowInsets.statusBars.asPaddingValues().calculateTopPadding() + PLAYER_BAR
            Box(
                Modifier.fillMaxWidth().height(bar + SCRIM_FADE).background(
                    Brush.verticalGradient(
                        0f to Color.Black.copy(alpha = SCRIM),
                        bar / (bar + SCRIM_FADE) to Color.Black.copy(alpha = SCRIM),
                        1f to Color.Transparent,
                    ),
                ),
            )
        }
    }
}

/** The player's top bar, without the status bar above it. */
private val PLAYER_BAR = 52.dp

/** How much of a cover shows behind the top bar: white words at 70% still read on a white one. */
private const val SCRIM = 0.72f
private val SCRIM_FADE = 88.dp

@Composable
private fun sleepTint(): Color {
    val timer by LocalContext.current.graph.sleep.endsAt.collectAsStateWithLifecycle()
    return if (timer != null) LocalPalette.current.accent else Color.White
}

/** Speed: its icon at normal speed, and the speed itself, in the accent, at any other. */
@Composable
private fun SpeedButton(speed: Float, onClick: () -> Unit) {
    if (speed == 1f) {
        GlyphButton(R.drawable.ic_sliders_vertical, "Speed", onClick)
    } else {
        val label = "%.2f×".format(Locale.US, speed).replace(".00×", "×")
        Box(
            Modifier.size(48.dp).clip(CircleShape).clickable(onClickLabel = "Speed: $label", role = Role.Button, onClick = onClick),
            contentAlignment = Alignment.Center,
        ) {
            Text(label, style = Type.tiny, color = LocalPalette.current.accent, maxLines = 1)
        }
    }
}

/**
 * The seek bar and its times. After a seek it stays where it was let go until the
 * engine has got there. A tap on the right-hand time changes it between what is
 * left and the song's whole length. [id] is the song's: a new one forgets the hold.
 */
@Composable
internal fun Seekbar(modifier: Modifier, id: String) {
    val graph = LocalContext.current.graph
    val scope = rememberCoroutineScope()
    val (position, duration) = rememberPosition()
    val settings by graph.settings.settings.collectAsStateWithLifecycle(Settings())
    var dragging by remember { mutableStateOf(false) }
    var dragValue by remember { mutableFloatStateOf(0f) }
    val hold = remember(id) { SeekHold() }
    val at = if (dragging) (dragValue * duration).toLong() else hold.shown(position, System.currentTimeMillis())
    val shown = if (duration > 0) at.toFloat() / duration else 0f
    Column(modifier) {
        ThinSeekBar(
            fraction = shown.coerceIn(0f, 1f),
            onDrag = { dragging = true; dragValue = it },
            onRelease = {
                if (duration > 0) {
                    val to = (dragValue * duration).toLong()
                    hold.sent(to, System.currentTimeMillis())
                    graph.playback.seek(to)
                }
                dragging = false
            },
        )
        Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
            Text(clock(at), style = Type.tiny, color = Color.White.copy(alpha = 0.7f))
            Spacer(Modifier.weight(1f))
            // Spotify counts down what is left.
            Text(
                if (settings.seekShowsRemaining) "-" + clock((duration - at).coerceAtLeast(0)) else clock(duration),
                style = Type.tiny, color = Color.White.copy(alpha = 0.7f),
                modifier = Modifier
                    .clickable(
                        onClickLabel = if (settings.seekShowsRemaining) "Show the song's length" else "Show the time left",
                        role = Role.Button,
                    ) {
                        scope.launch { graph.settings.update { it.copy(seekShowsRemaining = !it.seekShowsRemaining) } }
                    }
                    .padding(start = 12.dp, top = 4.dp, bottom = 4.dp),
            )
        }
    }
}

/**
 * Spotify's seek bar: a thin line with a small dot that grows while it is
 * held. A tap or a drag along it moves to that point; a touch that turns into a
 * scroll of the page leaves it alone.
 */
@Composable
private fun ThinSeekBar(fraction: Float, onDrag: (Float) -> Unit, onRelease: () -> Unit) {
    var held by remember { mutableStateOf(false) }
    val dot by animateFloatAsState(if (held) 8f else 6f, label = "dot")
    Canvas(
        Modifier.fillMaxWidth().padding(vertical = 2.dp).height(24.dp)
            .scrubSemantics(fraction, "Seek", onDrag, onRelease)
            .scrub(onHeld = { held = it }, onMove = onDrag, onDone = onRelease),
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
    val waiting = state.waiting()
    Row(
        Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        GlyphButton(R.drawable.ic_shuffle, if (shuffle) "Shuffle on" else "Shuffle off", { playback.setShuffle(!shuffle) }, tint = if (shuffle) p.accent else Color.White)
        GlyphButton(R.drawable.ic_skip_back_filled, "Previous", playback::previous, tint = Color.White, size = 32.dp)
        Box(
            Modifier.size(64.dp).clip(CircleShape).background(Color.White)
                .clickable(onClickLabel = if (playing) "Pause" else "Play", role = Role.Button, onClick = playback::toggle)
                .semantics { if (waiting) stateDescription = "Loading" },
            contentAlignment = Alignment.Center,
        ) {
            PlayGlyph(playing, waiting, Color.Black, 28.dp)
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
