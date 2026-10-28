package dev.darksavci.spotified.ui.components

import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.geometry.Offset
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.gestures.detectHorizontalDragGestures
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.runtime.derivedStateOf
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.annotation.DrawableRes
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.palette.graphics.Palette as ArtPalette
import coil3.compose.AsyncImage
import coil3.imageLoader
import coil3.request.ImageRequest
import coil3.request.SuccessResult
import coil3.request.allowHardware
import coil3.request.transformations
import coil3.toBitmap
import dev.darksavci.spotified.R
import dev.darksavci.spotified.api.Artwork
import dev.darksavci.spotified.api.Item
import dev.darksavci.spotified.api.Track
import dev.darksavci.spotified.api.url
import dev.darksavci.spotified.ui.theme.LocalPalette
import dev.darksavci.spotified.ui.theme.Type
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch

// ---------- icons ----------

/** A Lucide icon, the desktop's own (res/drawable/ic_*). */
@Composable
fun Glyph(@DrawableRes icon: Int, description: String?, modifier: Modifier = Modifier, tint: Color = LocalPalette.current.text, size: Dp = 24.dp) {
    Icon(painterResource(icon), description, modifier.size(size), tint = tint)
}

/** An icon that is a button, with a touch target of at least 48 dp. */
@Composable
fun GlyphButton(
    @DrawableRes icon: Int,
    description: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    tint: Color = LocalPalette.current.text,
    size: Dp = 24.dp,
) {
    Box(
        modifier
            .size(48.dp)
            .clip(CircleShape)
            .clickable(onClickLabel = description, role = Role.Button, onClick = onClick),
        contentAlignment = Alignment.Center,
    ) { Glyph(icon, description, tint = tint, size = size) }
}

// ---------- artwork ----------

@Composable
fun Art(artwork: List<Artwork>, size: Dp, modifier: Modifier = Modifier, shape: Shape = RoundedCornerShape(4.dp)) {
    val px = with(LocalDensity.current) { size.roundToPx() }
    Box(modifier.size(size).clip(shape).background(LocalPalette.current.surface)) {
        val address = artwork.url(px.coerceAtLeast(60))
        if (address != null) {
            // A round picture is an artist's: their photo is often padded with black bands.
            val model = if (shape == CircleShape) {
                ImageRequest.Builder(LocalContext.current).data(address).transformations(TrimBorders).build()
            } else address
            AsyncImage(model, null, Modifier.fillMaxSize(), contentScale = ContentScale.Crop)
        } else {
            Glyph(R.drawable.ic_music, null, Modifier.align(Alignment.Center), tint = LocalPalette.current.dim, size = size / 3)
        }
    }
}

/**
 * The colour a picture is made of, for the gradient behind a page's header
 * and Now Playing: its darker vibrant swatch, or its most common one.
 */
@Composable
fun artColor(artwork: List<Artwork>, fallback: Color = LocalPalette.current.surfaceActive): Color {
    val context = LocalContext.current
    val address = artwork.url(120)
    var color by remember(address) { mutableStateOf(fallback) }
    LaunchedEffect(address) {
        if (address == null) return@LaunchedEffect
        val request = ImageRequest.Builder(context).data(address).allowHardware(false).build()
        val result = context.imageLoader.execute(request) as? SuccessResult ?: return@LaunchedEffect
        val palette = ArtPalette.from(result.image.toBitmap()).generate()
        val swatch = palette.darkVibrantSwatch ?: palette.dominantSwatch ?: palette.mutedSwatch
        swatch?.let { color = Color(it.rgb).copy(alpha = 1f) }
    }
    return color
}

/** A colour darkened to sit behind white text. */
fun Color.darkened(amount: Float = 0.45f): Color =
    Color(red * (1 - amount), green * (1 - amount), blue * (1 - amount), alpha)

@Suppress("unused")
private fun Color.argb() = toArgb()

// ---------- rows ----------

@OptIn(ExperimentalFoundationApi::class)
@Composable
fun TrackRow(
    track: Track,
    onClick: () -> Unit,
    onMenu: () -> Unit,
    modifier: Modifier = Modifier,
    playing: Boolean = false,
    number: Int? = null,
    showArt: Boolean = true,
    subtitle: String? = null,
    downloaded: Boolean = false,
    trailing: @Composable (() -> Unit)? = null,
) {
    val p = LocalPalette.current
    val alpha = if (track.playable) 1f else 0.4f
    Row(
        modifier
            .fillMaxWidth()
            .combinedClickable(onClick = onClick, onLongClick = onMenu, enabled = track.playable)
            .padding(start = 16.dp, end = 4.dp, top = 6.dp, bottom = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (number != null) {
            Text(
                number.toString(), style = Type.body, color = if (playing) p.accent else p.secondary,
                modifier = Modifier.width(28.dp),
            )
        }
        if (showArt) {
            Art(track.artwork, 48.dp)
            Spacer(Modifier.width(12.dp))
        }
        Column(Modifier.weight(1f)) {
            Text(
                track.title, style = Type.body, maxLines = 1, overflow = TextOverflow.Ellipsis,
                color = (if (playing) p.accent else p.text).copy(alpha = alpha),
            )
            Row(verticalAlignment = Alignment.CenterVertically) {
                if (downloaded) {
                    Glyph(R.drawable.ic_circle_check, "Downloaded", tint = p.accent, size = 14.dp)
                    Spacer(Modifier.width(4.dp))
                }
                if (track.explicit) {
                    ExplicitBadge()
                    Spacer(Modifier.width(4.dp))
                }
                Text(
                    subtitle ?: track.artistNames, style = Type.small, color = p.secondary.copy(alpha = alpha),
                    maxLines = 1, overflow = TextOverflow.Ellipsis,
                )
            }
        }
        trailing?.invoke()
        GlyphButton(R.drawable.ic_ellipsis, "More for ${track.title}", onMenu, tint = p.secondary, size = 20.dp)
    }
}

@Composable
fun ExplicitBadge() {
    val p = LocalPalette.current
    Box(
        Modifier.clip(RoundedCornerShape(2.dp)).background(p.secondary).padding(horizontal = 4.dp),
        contentAlignment = Alignment.Center,
    ) { Text("E", style = Type.tiny, color = p.window) }
}

/** A list row for an album, artist, playlist or podcast. */
@Composable
fun EntityRow(
    title: String,
    subtitle: String,
    artwork: List<Artwork>,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    round: Boolean = false,
    pinned: Boolean = false,
    onLongClick: (() -> Unit)? = null,
) {
    val p = LocalPalette.current
    @OptIn(ExperimentalFoundationApi::class)
    Row(
        modifier
            .fillMaxWidth()
            .combinedClickable(onClick = onClick, onLongClick = onLongClick)
            .padding(horizontal = 16.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Art(artwork, 64.dp, shape = if (round) CircleShape else RoundedCornerShape(4.dp))
        Spacer(Modifier.width(12.dp))
        Column(Modifier.weight(1f)) {
            Text(title, style = Type.bodyStrong, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Row(verticalAlignment = Alignment.CenterVertically) {
                if (pinned) {
                    Glyph(R.drawable.ic_pin, "Pinned", tint = p.accent, size = 14.dp)
                    Spacer(Modifier.width(4.dp))
                }
                Text(subtitle, style = Type.small, color = p.secondary, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
        }
    }
}

// ---------- sliders ----------

/**
 * A slider drawn as Spotify draws one: a thin line in the accent colour and
 * a small dot that grows while it is held. A tap or a drag sets it; [onDone]
 * is called when the finger lifts.
 */
@Composable
fun ThinSlider(
    value: Float,
    onChange: (Float) -> Unit,
    onDone: () -> Unit,
    range: ClosedFloatingPointRange<Float>,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
) {
    val p = LocalPalette.current
    var held by remember { mutableStateOf(false) }
    val span = range.endInclusive - range.start
    val fraction = ((value - range.start) / span).coerceIn(0f, 1f)
    fun at(x: Float, width: Int) = range.start + span * (x / width).coerceIn(0f, 1f)
    val color = if (enabled) p.accent else p.dim
    Canvas(
        modifier.fillMaxWidth().height(32.dp)
            .pointerInput(enabled, range) {
                if (!enabled) return@pointerInput
                detectTapGestures(onPress = { o ->
                    held = true
                    onChange(at(o.x, size.width))
                    tryAwaitRelease()
                    held = false
                    onDone()
                })
            }
            .pointerInput(enabled, range) {
                if (!enabled) return@pointerInput
                detectHorizontalDragGestures(
                    onDragStart = { held = true },
                    onDragEnd = { held = false; onDone() },
                    onDragCancel = { held = false; onDone() },
                ) { change, _ -> onChange(at(change.position.x, size.width)) }
            },
    ) {
        val y = size.height / 2
        val stroke = 4.dp.toPx()
        drawLine(p.surfaceActive, Offset(0f, y), Offset(size.width, y), stroke, StrokeCap.Round)
        drawLine(color, Offset(0f, y), Offset(size.width * fraction, y), stroke, StrokeCap.Round)
        drawCircle(color, (if (held) 9 else 7).dp.toPx(), Offset(size.width * fraction, y))
    }
}

// ---------- page tops ----------

/**
 * The bar over an album, playlist, podcast or artist page: only the back
 * button at first, then, as the page scrolls past its header, the art's
 * colour and the title fade in behind it, as Spotify's do.
 */
@Composable
fun FadingTopBar(title: String, tint: Color, list: LazyListState, onBack: () -> Unit) {
    val density = LocalDensity.current
    val shown by remember(list) {
        derivedStateOf {
            if (list.firstVisibleItemIndex > 0) 1f
            else with(density) { ((list.firstVisibleItemScrollOffset - 220.dp.toPx()) / 120.dp.toPx()).coerceIn(0f, 1f) }
        }
    }
    Row(
        Modifier.fillMaxWidth().background(tint.darkened(0.45f).copy(alpha = shown))
            .windowInsetsPadding(WindowInsets.statusBars).padding(horizontal = 4.dp, vertical = 2.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        GlyphButton(R.drawable.ic_chevron_left, "Back", onBack, tint = Color.White)
        Text(
            title, style = Type.bodyStrong, color = Color.White.copy(alpha = shown), maxLines = 1,
            overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f), textAlign = TextAlign.Center,
        )
        Spacer(Modifier.width(48.dp))
    }
}

// ---------- cards and shelves ----------

@Composable
fun CoverCard(
    title: String,
    subtitle: String,
    artwork: List<Artwork>,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    size: Dp = 150.dp,
    round: Boolean = false,
    onLongClick: (() -> Unit)? = null,
) {
    val p = LocalPalette.current
    @OptIn(ExperimentalFoundationApi::class)
    Column(
        modifier
            .width(size)
            .clip(RoundedCornerShape(6.dp))
            .combinedClickable(onClick = onClick, onLongClick = onLongClick),
    ) {
        Art(artwork, size, shape = if (round) CircleShape else RoundedCornerShape(6.dp))
        Spacer(Modifier.height(8.dp))
        Text(title, style = Type.label, color = p.text, maxLines = 2, overflow = TextOverflow.Ellipsis)
        if (subtitle.isNotEmpty()) {
            Text(subtitle, style = Type.small, color = p.secondary, maxLines = 2, overflow = TextOverflow.Ellipsis)
        }
    }
}

@Composable
fun SectionTitle(text: String, modifier: Modifier = Modifier, action: String? = null, onAction: (() -> Unit)? = null) {
    Row(
        modifier.fillMaxWidth().padding(start = 16.dp, end = 8.dp, top = 24.dp, bottom = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(text, style = Type.heading, modifier = Modifier.weight(1f), maxLines = 2, overflow = TextOverflow.Ellipsis)
        if (action != null && onAction != null) {
            Text(
                action, style = Type.label, color = LocalPalette.current.secondary,
                modifier = Modifier.clip(RoundedCornerShape(12.dp)).clickable(onClick = onAction).padding(8.dp),
            )
        }
    }
}

/** A row of cards that scrolls sideways, as Home's shelves are. */
@Composable
fun ShelfRow(items: List<Item>, onOpen: (Item) -> Unit, onMenu: (Item) -> Unit) {
    LazyRow(contentPadding = PaddingValues(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(14.dp)) {
        items(items) { item ->
            CoverCard(
                title = item.title,
                subtitle = subtitleOf(item),
                artwork = item.artwork,
                round = item is Item.ArtistItem,
                onClick = { onOpen(item) },
                onLongClick = { onMenu(item) },
            )
        }
    }
}

fun subtitleOf(item: Item): String = when (item) {
    is Item.Song -> item.track.artistNames
    is Item.Episode -> item.track.artistNames
    is Item.AlbumItem -> listOf(item.album.kind, item.album.artists.joinToString { it.name }, item.album.year).filter { it.isNotEmpty() }.joinToString(" • ")
    is Item.ArtistItem -> item.artist.subscribers.ifEmpty { "Artist" }
    is Item.PlaylistItem -> item.playlist.owner.ifEmpty { "Playlist" }
    is Item.PodcastItem -> item.podcast.author.ifEmpty { "Podcast" }
}

// ---------- controls ----------

@Composable
fun Chip(text: String, selected: Boolean, onClick: () -> Unit, modifier: Modifier = Modifier) {
    val p = LocalPalette.current
    Box(
        modifier
            .clip(RoundedCornerShape(16.dp))
            .background(if (selected) p.accent else p.surface)
            .clickable(onClick = onClick, role = Role.Tab)
            .padding(horizontal = 14.dp, vertical = 7.dp),
    ) { Text(text, style = Type.label, color = if (selected) p.onAccent else p.text) }
}

/** The big round play button in the accent colour, the one strong colour on a page. */
@Composable
fun PlayButton(onClick: () -> Unit, modifier: Modifier = Modifier, playing: Boolean = false, size: Dp = 56.dp) {
    val p = LocalPalette.current
    Box(
        modifier.size(size).clip(CircleShape).background(p.accent)
            .clickable(onClickLabel = if (playing) "Pause" else "Play", role = Role.Button, onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        Glyph(if (playing) R.drawable.ic_pause_filled else R.drawable.ic_play_filled, if (playing) "Pause" else "Play", tint = p.onAccent, size = size * 0.42f)
    }
}

@Composable
fun Pill(text: String, onClick: () -> Unit, modifier: Modifier = Modifier, @DrawableRes icon: Int? = null, primary: Boolean = false) {
    val p = LocalPalette.current
    // The page's main action is filled in the accent, larger; the rest are quiet.
    val content = if (primary) p.onAccent else p.text
    Row(
        modifier.clip(RoundedCornerShape(24.dp)).background(if (primary) p.accent else p.surface).clickable(onClick = onClick)
            .padding(horizontal = if (primary) 24.dp else 14.dp, vertical = if (primary) 12.dp else 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (icon != null) {
            Glyph(icon, null, tint = content, size = 16.dp)
            Spacer(Modifier.width(6.dp))
        }
        Text(text, style = if (primary) Type.bodyStrong else Type.label, color = content)
    }
}

@Composable
fun Loading(modifier: Modifier = Modifier) {
    Box(modifier.fillMaxSize().padding(48.dp), contentAlignment = Alignment.Center) {
        CircularProgressIndicator(color = LocalPalette.current.accent, strokeWidth = 3.dp, modifier = Modifier.size(32.dp))
    }
}

@Composable
fun Problem(message: String, onRetry: (() -> Unit)?, modifier: Modifier = Modifier) {
    val p = LocalPalette.current
    Column(modifier.fillMaxWidth().padding(32.dp), horizontalAlignment = Alignment.CenterHorizontally) {
        Glyph(R.drawable.ic_circle_alert, null, tint = p.secondary, size = 32.dp)
        Spacer(Modifier.height(12.dp))
        Text(message, style = Type.body, color = p.secondary)
        if (onRetry != null) {
            Spacer(Modifier.height(16.dp))
            Pill("Try again", onRetry)
        }
    }
}

/** Nothing to show, said plainly: a title and what would be here, no alarm. */
@Composable
fun Empty(text: Pair<String, String>, modifier: Modifier = Modifier) {
    val p = LocalPalette.current
    Column(modifier.fillMaxWidth().padding(horizontal = 32.dp, vertical = 48.dp), horizontalAlignment = Alignment.CenterHorizontally) {
        Text(text.first, style = Type.heading, textAlign = TextAlign.Center)
        Spacer(Modifier.height(8.dp))
        Text(text.second, style = Type.body, color = p.secondary, textAlign = TextAlign.Center)
    }
}

// ---------- loading a page ----------

sealed interface Load<out T> {
    object Pending : Load<Nothing>
    data class Ready<T>(val value: T) : Load<T>
    data class Failed(val message: String) : Load<Nothing>
}

/** Loads a page's content once, keeps it across a rotation, and loads again on request. */
class Loader<T>(private val fetch: suspend () -> T) : ViewModel() {
    private val _state = MutableStateFlow<Load<T>>(Load.Pending)
    val state: StateFlow<Load<T>> = _state

    init { reload() }

    fun reload(quietly: Boolean = false) {
        if (!quietly) _state.value = Load.Pending
        viewModelScope.launch {
            _state.value = runCatching { fetch() }.fold(
                { Load.Ready(it) },
                { Load.Failed(it.message ?: "That could not be loaded.") },
            )
        }
    }
}

@Composable
fun <T> rememberLoader(key: String, fetch: suspend () -> T): Loader<T> =
    viewModel(key = key) { Loader(fetch) }

/** Shows a load: a spinner, the problem with a retry, or [content]. */
@Composable
fun <T> Loaded(loader: Loader<T>, content: @Composable (T) -> Unit) {
    val state by loader.state.collectAsStateWithLifecycle()
    when (val s = state) {
        Load.Pending -> Loading()
        is Load.Failed -> Problem(s.message, { loader.reload() })
        is Load.Ready -> content(s.value)
    }
}
