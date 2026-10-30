package dev.darksavci.spotified.ui.components

import dev.darksavci.spotified.ui.MainActivity
import dev.darksavci.spotified.api.ApiError
import android.content.Intent
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.geometry.Offset
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.runtime.derivedStateOf
import androidx.annotation.DrawableRes
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
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
import androidx.compose.material3.minimumInteractiveComponentSize
import dev.darksavci.spotified.api.FreshRead
import kotlinx.coroutines.withContext
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.State
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
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
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.onLongClick
import androidx.compose.ui.semantics.selected as selectedState
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
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
import dev.darksavci.spotified.graph
import dev.darksavci.spotified.ui.Change
import dev.darksavci.spotified.ui.Changes
import dev.darksavci.spotified.ui.theme.LocalPalette
import dev.darksavci.spotified.ui.theme.Type
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
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
            .tapRow(onClick, role = Role.Button, onClickLabel = description),
        contentAlignment = Alignment.Center,
    ) { Glyph(icon, description, tint = tint, size = size) }
}

// ---------- artwork ----------

/**
 * A cover or an artist's picture. Leave [description] out where the words
 * beside it already name it, so TalkBack does not read it twice.
 */
@Composable
fun Art(
    artwork: List<Artwork>,
    size: Dp,
    modifier: Modifier = Modifier,
    shape: Shape = RoundedCornerShape(4.dp),
    description: String? = null,
) {
    val px = with(LocalDensity.current) { size.roundToPx() }
    Box(modifier.size(size).clip(shape).background(LocalPalette.current.surface)) {
        val address = artwork.url(px.coerceAtLeast(60))
        if (address != null) {
            // A round picture is an artist's: their photo is often padded with black bands.
            val model = if (shape == CircleShape) {
                ImageRequest.Builder(LocalContext.current).data(address).transformations(TrimBorders).build()
            } else address
            AsyncImage(model, description, Modifier.fillMaxSize(), contentScale = ContentScale.Crop)
        } else {
            Glyph(R.drawable.ic_music, description, Modifier.align(Alignment.Center), tint = LocalPalette.current.dim, size = size / 3)
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

/** The id of the song playing now, held by Root, so that every row can tell without following the player itself. */
val LocalPlayingId = staticCompositionLocalOf<State<String?>> { mutableStateOf(null) }

/** Why a song is grey, for its row, its menu and TalkBack; null when it can be played. */
fun unplayableReason(track: Track): String? =
    if (track.playable) null else "Not available to play"

/**
 * A song. It marks itself when it is the one playing; [playing] overrides that
 * where the current song is not the point, as in the queue. A song that cannot
 * be played does nothing on a tap, says why in place of its artists, and keeps
 * its long press and menu, so that it can still be removed or looked into.
 */
@OptIn(ExperimentalFoundationApi::class)
@Composable
fun TrackRow(
    track: Track,
    onClick: () -> Unit,
    onMenu: () -> Unit,
    modifier: Modifier = Modifier,
    playing: Boolean? = null,
    number: Int? = null,
    showArt: Boolean = true,
    subtitle: String? = null,
    downloaded: Boolean = false,
    trailing: @Composable (() -> Unit)? = null,
    /** What a long press does, where it is not the menu: entering a selection. */
    onLongPress: (() -> Unit)? = null,
    /** Whether it is chosen, while a list is being selected from; a tap then chooses and the menu button goes. */
    selected: Boolean? = null,
) {
    val p = LocalPalette.current
    val alpha = if (track.playable) 1f else 0.4f
    val playingId by LocalPlayingId.current
    val current = playing ?: (track.id.isNotEmpty() && track.id == playingId)
    val reason = unplayableReason(track)
    val haptics = rememberHaptics()
    val hold = onLongPress ?: onMenu
    val menu by rememberUpdatedState(hold)
    val press = if (reason == null || selected != null) {
        Modifier.tapRow(
            onClick, hold,
            onClickLabel = if (selected == null) "Play" else if (selected) "Unselect" else "Select",
            onLongClickLabel = if (onLongPress != null) "Select" else "More options",
        )
    } else {
        Modifier
            .semantics(mergeDescendants = true) { onLongClick(if (onLongPress != null) "Select" else "More options") { menu(); true } }
            .pointerInput(Unit) { detectTapGestures(onLongPress = { haptics.longPress(); menu() }) }
    }
    Row(
        modifier
            .fillMaxWidth()
            .background(if (selected == true) p.surface else Color.Transparent)
            .then(press)
            .semantics {
                if (selected != null) selectedState = selected
                val state = reason ?: if (current) "Now playing" else null
                if (state != null) stateDescription = state
            }
            .padding(start = 16.dp, end = 4.dp, top = 6.dp, bottom = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (number != null) {
            Box(Modifier.width(28.dp)) {
                if (current) PlayingMark()
                else Text(number.toString(), style = Type.body, color = p.secondary)
            }
        }
        if (showArt) {
            Art(track.artwork, 48.dp)
            Spacer(Modifier.width(12.dp))
        }
        Column(Modifier.weight(1f)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                if (current && number == null) {
                    PlayingMark()
                    Spacer(Modifier.width(6.dp))
                }
                Text(
                    track.title, style = Type.body, maxLines = 1, overflow = TextOverflow.Ellipsis,
                    color = (if (current) p.accent else p.text).copy(alpha = alpha),
                )
            }
            Row(verticalAlignment = Alignment.CenterVertically) {
                if (downloaded) {
                    Glyph(R.drawable.ic_downloaded, "Downloaded", tint = p.accent, size = 13.dp)
                    Spacer(Modifier.width(4.dp))
                }
                if (track.explicit) {
                    ExplicitBadge()
                    Spacer(Modifier.width(4.dp))
                }
                Text(
                    reason ?: subtitle ?: track.artistNames, style = Type.small, color = p.secondary.copy(alpha = alpha),
                    maxLines = 1, overflow = TextOverflow.Ellipsis,
                )
            }
        }
        trailing?.invoke()
        if (selected == null) {
            GlyphButton(R.drawable.ic_ellipsis, "More for ${track.title}", onMenu, tint = p.secondary, size = 20.dp)
        } else {
            Box(Modifier.size(48.dp), contentAlignment = Alignment.Center) {
                Glyph(if (selected) R.drawable.ic_circle_check else R.drawable.ic_circle, null, tint = if (selected) p.accent else p.secondary, size = 22.dp)
            }
        }
    }
}

/** The small mark beside the song playing. TalkBack hears it as the row's state instead. */
@Composable
private fun PlayingMark() = Glyph(R.drawable.ic_audio_lines, null, tint = LocalPalette.current.accent, size = 16.dp)

@Composable
fun ExplicitBadge() {
    val p = LocalPalette.current
    Box(
        Modifier.clip(RoundedCornerShape(2.dp)).background(p.secondary).padding(horizontal = 4.dp)
            .clearAndSetSemantics { contentDescription = "Explicit" },
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
    artSize: Dp = 64.dp,
) {
    val p = LocalPalette.current
    Row(
        modifier
            .fillMaxWidth()
            .tapRow(onClick, onLongClick, role = Role.Button, onClickLabel = "Open", onLongClickLabel = "More options")
            .padding(horizontal = 16.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Art(artwork, artSize, shape = if (round) CircleShape else RoundedCornerShape(4.dp))
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
 * a small dot that grows while it is dragged. A tap or a drag sets it, a touch
 * that becomes a page scroll does not; [onDone] is called once when it is let go.
 * [description] is what TalkBack calls it.
 */
@Composable
fun ThinSlider(
    value: Float,
    onChange: (Float) -> Unit,
    onDone: () -> Unit,
    range: ClosedFloatingPointRange<Float>,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    description: String = "Slider",
) {
    val p = LocalPalette.current
    var held by remember { mutableStateOf(false) }
    val span = range.endInclusive - range.start
    val fraction = if (span > 0f) ((value - range.start) / span).coerceIn(0f, 1f) else 0f
    val color = if (enabled) p.accent else p.dim
    val set = { along: Float -> onChange(range.start + span * along) }
    Canvas(
        modifier.fillMaxWidth().height(32.dp)
            .scrubSemantics(fraction, description, set, onDone, enabled)
            .scrub(enabled, onHeld = { held = it }, onMove = set, onDone = onDone),
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
    /** How many lines its words may take; a small card in a grid keeps them few. */
    titleLines: Int = 2,
    subtitleLines: Int = 2,
) {
    val p = LocalPalette.current
    Column(
        modifier
            .width(size)
            .clip(RoundedCornerShape(6.dp))
            .tapRow(onClick, onLongClick, role = Role.Button, onClickLabel = "Open", onLongClickLabel = "More options"),
    ) {
        Art(artwork, size, shape = if (round) CircleShape else RoundedCornerShape(6.dp))
        Spacer(Modifier.height(8.dp))
        Text(title, style = Type.label, color = p.text, maxLines = titleLines, overflow = TextOverflow.Ellipsis)
        if (subtitle.isNotEmpty()) {
            Text(subtitle, style = Type.small, color = p.secondary, maxLines = subtitleLines, overflow = TextOverflow.Ellipsis)
        }
    }
}

@Composable
fun SectionTitle(text: String, modifier: Modifier = Modifier, action: String? = null, onAction: (() -> Unit)? = null) {
    Row(
        modifier.fillMaxWidth().padding(start = 16.dp, end = 8.dp, top = 24.dp, bottom = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(text, style = Type.heading, modifier = Modifier.weight(1f).semantics { heading() }, maxLines = 2, overflow = TextOverflow.Ellipsis)
        if (action != null && onAction != null) {
            Text(
                action, style = Type.label, color = LocalPalette.current.secondary,
                modifier = Modifier.clip(RoundedCornerShape(12.dp)).tapRow(onAction, role = Role.Button, onClickLabel = "$action: $text").padding(8.dp),
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

/**
 * A filter or choice. It reads as a selectable tab; give a chip that is an
 * action, such as a sort, [role] Button. [description] replaces its words for
 * TalkBack where they say too little.
 */
@Composable
fun Chip(
    text: String,
    selected: Boolean,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    role: Role = Role.Tab,
    description: String? = null,
    @DrawableRes icon: Int? = null,
) {
    val p = LocalPalette.current
    val content = if (selected) p.onAccent else p.text
    // The pill is drawn at its own size; the touch is the 48 dp around it.
    Box(
        modifier
            .minimumInteractiveComponentSize()
            .tapRow(onClick, role = role)
            .semantics {
                if (role == Role.Tab) selectedState = selected
                if (description != null) contentDescription = description
            },
        contentAlignment = Alignment.Center,
    ) {
        Row(
            Modifier
                .clip(RoundedCornerShape(16.dp))
                .background(if (selected) p.accent else p.surface)
                .padding(horizontal = 14.dp, vertical = 7.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            if (icon != null) {
                Glyph(icon, null, tint = content, size = 14.dp)
                Spacer(Modifier.width(6.dp))
            }
            Text(text, style = Type.label, color = content)
        }
    }
}

/** The big round play button in the accent colour, the one strong colour on a page. */
@Composable
fun PlayButton(onClick: () -> Unit, modifier: Modifier = Modifier, playing: Boolean = false, size: Dp = 56.dp) {
    val p = LocalPalette.current
    Box(
        modifier.size(size).clip(CircleShape).background(p.accent)
            .tapRow(onClick, role = Role.Button, onClickLabel = if (playing) "Pause" else "Play"),
        contentAlignment = Alignment.Center,
    ) {
        Glyph(if (playing) R.drawable.ic_pause_filled else R.drawable.ic_play_filled, if (playing) "Pause" else "Play", tint = p.onAccent, size = size * 0.42f)
    }
}

@Composable
fun Pill(
    text: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    @DrawableRes icon: Int? = null,
    primary: Boolean = false,
    enabled: Boolean = true,
    description: String? = null,
) {
    val p = LocalPalette.current
    // The page's main action is filled in the accent, larger; the rest are quiet.
    val content = (if (primary) p.onAccent else p.text).copy(alpha = if (enabled) 1f else 0.5f)
    Row(
        modifier.clip(RoundedCornerShape(24.dp)).background(if (primary) p.accent else p.surface)
            .tapRow(onClick, enabled = enabled, role = Role.Button)
            .semantics { if (description != null) contentDescription = description }
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
        if (message == ApiError.SignedOut.message) {
            // Trying again would only say the same.
            val context = LocalContext.current
            Spacer(Modifier.height(16.dp))
            Pill("Sign in", {
                context.startActivity(
                    Intent(MainActivity.ACTION_SIGN_IN, null, context, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_SINGLE_TOP),
                )
            }, primary = true)
        } else if (onRetry != null) {
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

/**
 * Loads a page's content once, keeps it across a rotation, and loads again on
 * request. Only the newest load counts: asking again drops one under way.
 */
class Loader<T>(private var fetch: suspend () -> T, scope: CoroutineScope? = null) : ViewModel() {
    private val scope = scope ?: viewModelScope
    private var job: Job? = null
    private val _state = MutableStateFlow<Load<T>>(Load.Pending)
    val state: StateFlow<Load<T>> = _state

    private val _refreshing = MutableStateFlow(false)

    /** Whether a [reload] that keeps the content is under way, for a pull's spinner. */
    val refreshing: StateFlow<Boolean> = _refreshing

    private val _failures = MutableSharedFlow<String>(extraBufferCapacity = 1)

    /** What went wrong when a quiet load failed and the content stayed. [rememberLoader] says it in a message. */
    val failures: SharedFlow<String> = _failures

    /** The [Changes] this page has seen; what it follows having moved past this means loading again. */
    private var seen = 0L

    init { reload() }

    /** Loads again. [quietly] keeps what shows, and a failure then leaves it and says so in [failures]. */
    fun reload(quietly: Boolean = false) = load(keep = quietly, report = quietly, fresh = quietly)

    /** Something this page shows was changed: loads again with no spinner, unless a pull's is showing. */
    fun refreshBehind() = load(keep = true, report = _refreshing.value)

    private fun load(keep: Boolean, report: Boolean, fresh: Boolean = false) {
        job?.cancel()
        if (!keep) _state.value = Load.Pending
        _refreshing.value = report
        job = scope.launch {
            // Only a pull asks past the core's kept answers; a write has already made them out of date.
            val result = runCatching { if (fresh) withContext(FreshRead) { fetch() } else fetch() }
            // A load cancelled is one replaced, or a page gone: it has nothing to say.
            currentCoroutineContext().ensureActive()
            result.fold(
                { _state.value = Load.Ready(it) },
                {
                    val message = it.message ?: "That could not be loaded."
                    if (keep && _state.value is Load.Ready) _failures.tryEmit("Could not refresh: $message")
                    else _state.value = Load.Failed(message)
                },
            )
            _refreshing.value = false
        }
    }

    private var question: Any? = null
    private var asked = false

    /**
     * For one loader kept for a page whose question changes, so that a search
     * does not leave a loader behind per query. Give it what the fetch reads; it loads again when that changes, keeping nothing, for
     * [keptState] to hold the old answer. The first question is the one [init] loaded.
     * Call it while composing, so that no frame shows the last question's answer.
     */
    fun askFor(now: Any?, latest: suspend () -> T) {
        if (asked && now == question) return
        val first = !asked
        asked = true
        question = now
        use(latest)
        if (!first) reload()
    }

    internal fun use(latest: suspend () -> T) {
        fetch = latest
    }

    /** Starts from [version], so that what was already changed before this loaded does not load it again. */
    internal fun seenAt(version: Long) = apply { seen = version }

    /** The version of what it follows has moved to [version]. */
    internal fun follow(version: Long) {
        if (version == seen) return
        seen = version
        refreshBehind()
    }
}

/**
 * The page's loader for [key]: a new key is a new load. With [follows], it loads
 * again behind what shows after a write says one of those changed, at once if the
 * page is showing and otherwise when it is returned to.
 */
@Composable
fun <T> rememberLoader(key: String, follows: Set<Change> = emptySet(), fetch: suspend () -> T): Loader<T> {
    val loader: Loader<T> = viewModel(key = key) { Loader(fetch).seenAt(Changes.version(follows)) }
    val context = LocalContext.current
    SideEffect { loader.use(fetch) }
    LaunchedEffect(loader) { loader.failures.collect { context.graph.playback.say(it) } }
    if (follows.isNotEmpty()) {
        LaunchedEffect(loader, follows) { Changes.follow(follows).collect { loader.follow(it) } }
    }
    return loader
}
