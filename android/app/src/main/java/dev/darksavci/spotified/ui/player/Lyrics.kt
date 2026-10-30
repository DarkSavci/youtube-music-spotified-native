package dev.darksavci.spotified.ui.player

import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
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
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.nestedscroll.NestedScrollConnection
import androidx.compose.ui.input.nestedscroll.NestedScrollSource
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import dev.darksavci.spotified.R
import dev.darksavci.spotified.api.LyricLine
import dev.darksavci.spotified.api.Lyrics
import dev.darksavci.spotified.api.PlayState
import dev.darksavci.spotified.api.Track
import dev.darksavci.spotified.data.LyricsExtras
import dev.darksavci.spotified.data.Settings
import dev.darksavci.spotified.graph
import dev.darksavci.spotified.ui.components.GlyphButton
import dev.darksavci.spotified.ui.components.Pill
import dev.darksavci.spotified.ui.components.darkened
import dev.darksavci.spotified.ui.theme.Type
import java.util.Locale
import kotlinx.coroutines.launch

/** Lines away from the one being sung, on the lyrics' dark background: still easy to read, but not lit. */
private val OTHER_LINE = Color.White.copy(alpha = 0.55f)

/** The words of the line being sung that are not the word being sung, so that the line still reads as the current one. */
private val OTHER_WORD = Color.White.copy(alpha = 0.78f)

/**
 * A line's text with only the word being sung left at the text's own colour and
 * the others at [others], for lines the source times word by word; the plain text
 * otherwise, which is lit as a whole. The word being sung is the last one begun. A
 * source can time the spaces too, and a space is never that word; before the first
 * word has begun, none is.
 */
internal fun sung(line: LyricLine, positionMs: Long, others: Color): AnnotatedString = buildAnnotatedString {
    if (line.words.isEmpty()) {
        append(line.text.ifEmpty { "♪" })
        return@buildAnnotatedString
    }
    val singing = line.words.indexOfLast { it.atMs <= positionMs && it.text.isNotBlank() }
    line.words.forEachIndexed { i, word ->
        if (i == singing) append(word.text)
        else withStyle(SpanStyle(color = others)) { append(word.text) }
    }
}

/**
 * The line to light at [positionMs]: none before the first line has begun, none in
 * lyrics with no times, and none on a line with no words, which is how a source marks
 * an instrumental gap. Without an end to each line, nothing else can show a gap.
 */
internal fun Lyrics.lineToLight(positionMs: Long): Int? {
    if (!synced) return null
    return lineAt(positionMs)?.takeIf { lines[it].text.isNotBlank() }
}

/** A song's lyrics as they load: [result] is null while loading, and holds null when there are none. */
private class LoadedLyrics(val result: Result<Lyrics?>?, val retry: () -> Unit) {
    val lyrics: Lyrics? get() = result?.getOrNull()
    val failed: Boolean get() = result?.isFailure == true
}

/** A song's lyrics, loaded once per song, or again on asking after a failure. */
@Composable
private fun rememberLyrics(track: Track): LoadedLyrics {
    val graph = LocalContext.current.graph
    var attempt by remember(track.id) { mutableIntStateOf(0) }
    val loaded by produceState<Result<Lyrics?>?>(null, track.id, attempt) {
        value = null
        value = runCatching { graph.api.lyrics(track) }
    }
    return LoadedLyrics(loaded) { attempt++ }
}

/** Said when a song's words could not be had; the one thing to do about it is to try again. */
@Composable
private fun LyricsFailed(onRetry: () -> Unit) {
    Text("The lyrics could not be loaded.", style = Type.body, color = Color.White.copy(alpha = 0.7f))
    Spacer(Modifier.height(12.dp))
    Pill("Try again", onRetry)
}

/**
 * The lyrics in the art's place, for the lyrics-first style: the current
 * line and the next few, following the song. A tap gives them the screen.
 */
@Composable
fun LyricsPanel(track: Track, size: Dp, tint: Color, onExpand: () -> Unit) {
    val loaded = rememberLyrics(track)
    val lyrics = loaded.lyrics
    val (position, _) = rememberPosition(if (lyrics.hasWords()) 50 else 250)
    val graph = LocalContext.current.graph
    val offsets by graph.settings.lyricsOffsets.collectAsStateWithLifecycle(emptyMap())
    val at = position + (offsets[track.id] ?: 0L)
    // Before the first line, the panel starts with it, not lit.
    val current = lyrics?.lineAt(at) ?: 0
    val lit = lyrics?.lineToLight(at)
    val lines = lyrics?.lines.orEmpty()
    Column(
        Modifier.size(size).clip(RoundedCornerShape(8.dp)).background(readableBehindWhite(tint.darkened(0.2f), 7.0))
            .clickable(enabled = lines.isNotEmpty(), onClickLabel = "Show the lyrics", onClick = onExpand).padding(20.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        when {
            loaded.result == null -> Text("Loading…", style = Type.title, color = Color.White.copy(alpha = 0.7f))
            loaded.failed -> LyricsFailed(loaded.retry)
            lines.isEmpty() -> Text("No lyrics for this song.", style = Type.title, color = Color.White.copy(alpha = 0.7f))
        }
        lines.drop(current).take(7).forEachIndexed { i, line ->
            val now = current + i == lit
            val color by animateColorAsState(if (now || lyrics?.synced != true) Color.White else OTHER_LINE, tween(LINE_MS), label = "line")
            Text(
                if (now) sung(line, at, OTHER_WORD) else AnnotatedString(line.text.ifEmpty { "♪" }),
                style = Type.title, maxLines = 2, overflow = TextOverflow.Ellipsis,
                color = color,
            )
        }
    }
}

/** The card under the controls: the first lines, and a button that gives them the screen. */
@Composable
fun LyricsCard(track: Track, tint: Color, onExpand: () -> Unit) {
    val loaded = rememberLyrics(track)
    val lyrics = loaded.lyrics
    val hasLines = lyrics != null && lyrics.lines.isNotEmpty()
    Column(
        Modifier.fillMaxWidth().padding(horizontal = 16.dp).clip(RoundedCornerShape(10.dp))
            .background(tint.darkened(0.2f)).clickable(enabled = hasLines, onClick = onExpand).padding(16.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text("Lyrics", style = Type.heading, color = Color.White, modifier = Modifier.weight(1f))
            if (hasLines) GlyphButton(R.drawable.ic_maximize_2, "Show the lyrics", onExpand, tint = Color.White, size = 20.dp)
        }
        Spacer(Modifier.height(8.dp))
        when {
            loaded.result == null -> Text("Loading…", style = Type.body, color = Color.White.copy(alpha = 0.7f))
            loaded.failed -> LyricsFailed(loaded.retry)
            !hasLines -> Text("No lyrics for this song.", style = Type.body, color = Color.White.copy(alpha = 0.7f))
            else -> Text(
                lyrics!!.lines.take(4).joinToString("\n") { it.text },
                style = Type.title, color = Color.White, maxLines = 4, overflow = TextOverflow.Ellipsis,
            )
        }
    }
}

/**
 * The lyrics given the screen, on the art's colour. Timed lines follow the song,
 * the current one near the top, until the listener scrolls them; then a button
 * goes back to the song. A tap on a line goes to it, and the song can be played,
 * paused and moved from here. Lines can be shown romanised or translated, nudged
 * earlier or later for a song whose timing is off, and chosen to share as a card.
 */
@Composable
fun LyricsScreen(track: Track, tint: Color, onClose: () -> Unit) {
    val graph = LocalContext.current.graph
    val context = LocalContext.current
    val settings by graph.settings.settings.collectAsStateWithLifecycle(Settings())
    val offsets by graph.settings.lyricsOffsets.collectAsStateWithLifecycle(emptyMap())
    val offset = offsets[track.id] ?: 0L
    val loaded = rememberLyrics(track)
    val lyrics = loaded.lyrics
    val (position, _) = rememberPosition(if (lyrics.hasWords()) 50 else 250)
    val list = rememberLazyListState()
    val scope = rememberCoroutineScope()
    var choosing by remember { mutableStateOf(false) }
    val chosen = remember { mutableStateListOf<Int>() }
    var following by remember(track.id) { mutableStateOf(true) }

    // Only what the list itself scrolled counts: a pull at its top moves the page, not the words.
    val scrolled = remember {
        object : NestedScrollConnection {
            override fun onPostScroll(consumed: Offset, available: Offset, source: NestedScrollSource): Offset {
                if (source == NestedScrollSource.UserInput && consumed.y != 0f) following = false
                return Offset.Zero
            }
        }
    }

    val extra by produceState<List<String>?>(null, lyrics, settings.lyricsTranslate, settings.lyricsRomanise) {
        value = if (lyrics == null) null else LyricsExtras.of(graph.http, lyrics.lines.map { it.text }, settings.lyricsRomanise, settings.lyricsTranslate)
    }
    val current = lyrics?.lineAt(position + offset)
    val lit = lyrics?.lineToLight(position + offset)
    LaunchedEffect(current, following, choosing) {
        if (current != null && following && !choosing) list.animateScrollToItem((current - 1).coerceAtLeast(0))
    }

    // Darker than the rest of the player, so that the lines still to come read as well, at their lower alpha.
    Box(Modifier.fillMaxSize().background(readableBehindWhite(tint, 7.0)).pointerInput(Unit) {}) {
        Column(Modifier.fillMaxSize().windowInsetsPadding(WindowInsets.safeDrawing)) {
            Row(Modifier.fillMaxWidth().padding(horizontal = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                GlyphButton(R.drawable.ic_chevron_down, "Close the lyrics", onClose, tint = Color.White)
                Column(Modifier.weight(1f), horizontalAlignment = Alignment.CenterHorizontally) {
                    if (choosing) {
                        Text("Choose up to 5 lines", style = Type.label, color = Color.White, maxLines = 1, overflow = TextOverflow.Ellipsis)
                        Text(if (chosen.isEmpty()) "Tap the lines to share" else "${chosen.size} chosen", style = Type.small, color = Color.White.copy(alpha = 0.7f), maxLines = 1)
                    } else {
                        Text(track.title, style = Type.label, color = Color.White, maxLines = 1, overflow = TextOverflow.Ellipsis)
                        Text(track.artistNames, style = Type.small, color = Color.White.copy(alpha = 0.7f), maxLines = 1)
                    }
                }
                if (lyrics != null && lyrics.lines.isNotEmpty() && !choosing) {
                    GlyphButton(R.drawable.ic_share_2, "Choose lines to share", { choosing = true }, tint = Color.White)
                } else {
                    Spacer(Modifier.width(48.dp))
                }
            }
            Box(Modifier.weight(1f).fillMaxWidth().nestedScroll(scrolled)) {
                when {
                    loaded.result == null -> Text("Loading…", Modifier.padding(24.dp), style = Type.title, color = Color.White.copy(alpha = 0.7f))
                    loaded.failed -> Column(Modifier.padding(24.dp)) { LyricsFailed(loaded.retry) }
                    lyrics == null || lyrics.lines.isEmpty() ->
                        Text("No lyrics for this song.", Modifier.padding(24.dp), style = Type.title, color = Color.White.copy(alpha = 0.7f))
                    else -> LazyColumn(
                        state = list, modifier = Modifier.fillMaxSize(),
                        contentPadding = PaddingValues(horizontal = 24.dp, vertical = 16.dp),
                        verticalArrangement = Arrangement.spacedBy(14.dp),
                    ) {
                        itemsIndexed(lyrics.lines) { i, line ->
                            val now = i == lit
                            // With no times, nothing is being sung and every line is as bright as the next.
                            val color by animateColorAsState(if (now || !lyrics.synced) Color.White else OTHER_LINE, tween(LINE_MS), label = "line")
                            val picked = i in chosen
                            Column(
                                Modifier.fillMaxWidth().clip(RoundedCornerShape(6.dp))
                                    .background(if (picked) Color.White.copy(alpha = 0.25f) else Color.Transparent)
                                    .clickable {
                                        when {
                                            choosing -> if (picked) chosen.remove(i) else if (chosen.size < 5) chosen.add(i)
                                            lyrics.synced -> {
                                                graph.playback.seek((line.atMs - offset).coerceAtLeast(0))
                                                following = true
                                            }
                                        }
                                    },
                            ) {
                                // Only the line being sung is lit, and in it the word being sung when the source says.
                                Text(
                                    if (now) sung(line, position + offset, OTHER_WORD)
                                    else AnnotatedString(line.text.ifEmpty { "♪" }),
                                    style = Type.title,
                                    color = color,
                                )
                                extra?.getOrNull(i)?.takeIf { it.isNotBlank() && it != line.text }?.let {
                                    Text(it, style = Type.body, color = if (now || !lyrics.synced) Color.White.copy(alpha = 0.85f) else OTHER_LINE)
                                }
                            }
                        }
                        item {
                            if (lyrics.source.isNotEmpty()) Text("Lyrics from ${lyrics.source}", style = Type.small, color = Color.White.copy(alpha = 0.6f))
                        }
                    }
                }
                if (!following && !choosing && current != null) {
                    Pill(
                        "Back to the song", { following = true },
                        Modifier.align(Alignment.BottomCenter).padding(bottom = 12.dp),
                        icon = R.drawable.ic_arrow_right,
                        primary = true,
                    )
                }
            }
            if (choosing) {
                Row(Modifier.fillMaxWidth().padding(16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                    Pill("Cancel", {
                        chosen.clear()
                        choosing = false
                    })
                    Spacer(Modifier.weight(1f))
                    Pill(
                        "Share",
                        {
                            if (lyrics != null && chosen.isNotEmpty()) {
                                LyricCard.share(context, track, chosen.sorted().map { lyrics.lines[it].text }, tint)
                                chosen.clear()
                                choosing = false
                            }
                        },
                        enabled = chosen.isNotEmpty(), primary = true,
                    )
                }
            } else {
                Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 4.dp), horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                    Pill(if (settings.lyricsRomanise) "Romanised" else "Romanise", {
                        scope.launch { graph.settings.update { it.copy(lyricsRomanise = !it.lyricsRomanise) } }
                    })
                    Pill(if (settings.lyricsTranslate) "Translated" else "Translate", {
                        scope.launch { graph.settings.update { it.copy(lyricsTranslate = !it.lyricsTranslate) } }
                    })
                    Spacer(Modifier.weight(1f))
                    if (lyrics?.synced == true) {
                        Pill("−", { scope.launch { graph.settings.setLyricsOffset(track.id, offset - 250) } }, description = "Lyrics earlier")
                        Text("%+.2fs".format(Locale.US, offset / 1000f), style = Type.small, color = Color.White, modifier = Modifier.width(52.dp))
                        Pill("+", { scope.launch { graph.settings.setLyricsOffset(track.id, offset + 250) } }, description = "Lyrics later")
                    }
                }
                Seekbar(Modifier.padding(horizontal = 24.dp), track.id)
                LyricsControls()
            }
        }
    }
}

/** Previous, play and next, so that the song can be run from the lyrics. */
@Composable
private fun LyricsControls() {
    val graph = LocalContext.current.graph
    val projection by graph.playback.projection.collectAsStateWithLifecycle()
    val playing = projection.target.playing
    val state = projection.state.state
    val waiting = state == PlayState.Loading || state == PlayState.Stalled
    Row(
        Modifier.fillMaxWidth().padding(bottom = 8.dp),
        horizontalArrangement = Arrangement.Center,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        GlyphButton(R.drawable.ic_skip_back_filled, "Previous", graph.playback::previous, tint = Color.White, size = 28.dp)
        Spacer(Modifier.width(24.dp))
        Box(
            Modifier.size(52.dp).clip(CircleShape).background(Color.White)
                .clickable(onClickLabel = if (playing) "Pause" else "Play", role = Role.Button, onClick = graph.playback::toggle)
                .semantics { if (waiting) stateDescription = "Loading" },
            contentAlignment = Alignment.Center,
        ) {
            PlayGlyph(playing, waiting, Color.Black, 24.dp)
        }
        Spacer(Modifier.width(24.dp))
        GlyphButton(R.drawable.ic_skip_forward_filled, "Next", graph.playback::next, tint = Color.White, size = 28.dp)
    }
}

private const val LINE_MS = 200

/** Whether any line is timed word by word, so the position is followed more closely. */
private fun Lyrics?.hasWords(): Boolean = this?.lines?.any { it.words.isNotEmpty() } == true
