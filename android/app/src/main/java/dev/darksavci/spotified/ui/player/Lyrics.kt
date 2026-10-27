package dev.darksavci.spotified.ui.player

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
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import dev.darksavci.spotified.R
import dev.darksavci.spotified.api.Lyrics
import dev.darksavci.spotified.api.Track
import dev.darksavci.spotified.data.LyricsExtras
import dev.darksavci.spotified.data.Settings
import dev.darksavci.spotified.graph
import dev.darksavci.spotified.ui.components.GlyphButton
import dev.darksavci.spotified.ui.components.Pill
import dev.darksavci.spotified.ui.components.darkened
import dev.darksavci.spotified.ui.theme.Type
import kotlinx.coroutines.launch

/** A song's lyrics, loaded once per song; null while loading, empty when there are none. */
@Composable
private fun rememberLyrics(track: Track): Result<Lyrics?>? {
    val graph = LocalContext.current.graph
    val loaded by produceState<Result<Lyrics?>?>(null, track.id) {
        value = runCatching { graph.api.lyrics(track) }
    }
    return loaded
}

/**
 * The lyrics in the art's place, for the lyrics-first style: the current
 * line and the next few, following the song. A tap gives them the screen.
 */
@Composable
fun LyricsPanel(track: Track, size: Dp, tint: Color, onExpand: () -> Unit) {
    val lyrics = rememberLyrics(track)?.getOrNull()
    val (position, _) = rememberPosition()
    val graph = LocalContext.current.graph
    val offsets by graph.settings.lyricsOffsets.collectAsStateWithLifecycle(emptyMap())
    val current = lyrics?.lineAt(position + (offsets[track.id] ?: 0L)) ?: 0
    Column(
        Modifier.size(size).clip(RoundedCornerShape(8.dp)).background(tint.darkened(0.2f)).clickable(onClick = onExpand).padding(20.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        val lines = lyrics?.lines.orEmpty()
        if (lines.isEmpty()) {
            Text(if (lyrics == null) "No lyrics for this song." else "", style = Type.title, color = Color.White.copy(alpha = 0.7f))
        }
        lines.drop(current).take(7).forEachIndexed { i, line ->
            Text(
                line.text.ifEmpty { "♪" }, style = Type.title, maxLines = 2, overflow = TextOverflow.Ellipsis,
                color = if (i == 0 && lyrics?.synced == true) Color.White else Color.White.copy(alpha = 0.5f),
            )
        }
    }
}

/** The card under the controls: the first lines, and a button that gives them the screen. */
@Composable
fun LyricsCard(track: Track, tint: Color, onExpand: () -> Unit) {
    val loaded = rememberLyrics(track)
    val lyrics = loaded?.getOrNull()
    Column(
        Modifier.fillMaxWidth().padding(horizontal = 16.dp).clip(RoundedCornerShape(10.dp))
            .background(tint.darkened(0.2f)).clickable(enabled = lyrics != null, onClick = onExpand).padding(16.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text("Lyrics", style = Type.heading, color = Color.White, modifier = Modifier.weight(1f))
            if (lyrics != null) GlyphButton(R.drawable.ic_maximize_2, "Show the lyrics", onExpand, tint = Color.White, size = 20.dp)
        }
        Spacer(Modifier.height(8.dp))
        when {
            loaded == null -> Text("Loading…", style = Type.body, color = Color.White.copy(alpha = 0.7f))
            loaded.isFailure -> Text("The lyrics could not be loaded.", style = Type.body, color = Color.White.copy(alpha = 0.7f))
            lyrics == null || lyrics.lines.isEmpty() -> Text("No lyrics for this song.", style = Type.body, color = Color.White.copy(alpha = 0.7f))
            else -> Text(
                lyrics.lines.take(4).joinToString("\n") { it.text },
                style = Type.title, color = Color.White, maxLines = 4, overflow = TextOverflow.Ellipsis,
            )
        }
    }
}

/**
 * The lyrics given the screen, on the art's colour. Timed lines follow the
 * song, the current one near the top; a tap on a line goes to it. Lines can
 * be shown romanised or translated, nudged earlier or later for a song whose
 * timing is off, and chosen to share as a card.
 */
@Composable
fun LyricsScreen(track: Track, tint: Color, onClose: () -> Unit) {
    val graph = LocalContext.current.graph
    val context = LocalContext.current
    val settings by graph.settings.settings.collectAsStateWithLifecycle(Settings())
    val offsets by graph.settings.lyricsOffsets.collectAsStateWithLifecycle(emptyMap())
    val offset = offsets[track.id] ?: 0L
    val loaded = rememberLyrics(track)
    val lyrics = loaded?.getOrNull()
    val (position, _) = rememberPosition()
    val list = rememberLazyListState()
    val scope = rememberCoroutineScope()
    var choosing by remember { mutableStateOf(false) }
    val chosen = remember { mutableStateListOf<Int>() }

    val extra by produceState<List<String>?>(null, lyrics, settings.lyricsTranslate, settings.lyricsRomanise) {
        value = if (lyrics == null) null else LyricsExtras.of(graph.http, lyrics.lines.map { it.text }, settings.lyricsRomanise, settings.lyricsTranslate)
    }
    val current = lyrics?.lineAt(position + offset)
    LaunchedEffect(current) {
        if (current != null && !choosing) list.animateScrollToItem((current - 1).coerceAtLeast(0))
    }

    Box(Modifier.fillMaxSize().background(tint.darkened(0.15f)).clickable(enabled = false) {}) {
        Column(Modifier.fillMaxSize().windowInsetsPadding(WindowInsets.safeDrawing)) {
            Row(Modifier.fillMaxWidth().padding(horizontal = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                GlyphButton(R.drawable.ic_chevron_down, "Close the lyrics", onClose, tint = Color.White)
                Column(Modifier.weight(1f), horizontalAlignment = Alignment.CenterHorizontally) {
                    Text(track.title, style = Type.label, color = Color.White, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    Text(track.artistNames, style = Type.small, color = Color.White.copy(alpha = 0.7f), maxLines = 1)
                }
                GlyphButton(R.drawable.ic_share_2, if (choosing) "Share the chosen lines" else "Choose lines to share", {
                    if (choosing && chosen.isNotEmpty() && lyrics != null) {
                        LyricCard.share(context, track, chosen.sorted().map { lyrics.lines[it].text }, tint)
                        chosen.clear()
                        choosing = false
                    } else {
                        choosing = !choosing
                    }
                }, tint = if (choosing) Color.Black else Color.White)
            }
            when {
                loaded == null -> Text("Loading…", Modifier.padding(24.dp), style = Type.title, color = Color.White.copy(alpha = 0.7f))
                lyrics == null || lyrics.lines.isEmpty() ->
                    Text("No lyrics for this song.", Modifier.padding(24.dp), style = Type.title, color = Color.White.copy(alpha = 0.7f))
                else -> LazyColumn(
                    state = list, modifier = Modifier.weight(1f),
                    contentPadding = PaddingValues(horizontal = 24.dp, vertical = 16.dp),
                    verticalArrangement = Arrangement.spacedBy(14.dp),
                ) {
                    itemsIndexed(lyrics.lines) { i, line ->
                        val past = current != null && i <= current
                        val picked = i in chosen
                        Column(
                            Modifier.fillMaxWidth().clip(RoundedCornerShape(6.dp))
                                .background(if (picked) Color.White.copy(alpha = 0.25f) else Color.Transparent)
                                .clickable {
                                    when {
                                        choosing -> if (picked) chosen.remove(i) else if (chosen.size < 5) chosen.add(i)
                                        lyrics.synced -> graph.playback.seek((line.atMs - offset).coerceAtLeast(0))
                                    }
                                },
                        ) {
                            Text(
                                line.text.ifEmpty { "♪" }, style = Type.title,
                                color = if (!lyrics.synced || past) Color.White else Color.White.copy(alpha = 0.45f),
                            )
                            extra?.getOrNull(i)?.takeIf { it.isNotBlank() && it != line.text }?.let {
                                Text(it, style = Type.body, color = Color.White.copy(alpha = if (past || !lyrics.synced) 0.85f else 0.4f))
                            }
                        }
                    }
                    item {
                        if (lyrics.source.isNotEmpty()) Text("Lyrics from ${lyrics.source}", style = Type.small, color = Color.White.copy(alpha = 0.6f))
                    }
                }
            }
            Row(Modifier.fillMaxWidth().padding(16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                Pill(if (settings.lyricsRomanise) "Romanised" else "Romanise", {
                    scope.launch { graph.settings.update { it.copy(lyricsRomanise = !it.lyricsRomanise) } }
                })
                Pill(if (settings.lyricsTranslate) "Translated" else "Translate", {
                    scope.launch { graph.settings.update { it.copy(lyricsTranslate = !it.lyricsTranslate) } }
                })
                Spacer(Modifier.weight(1f))
                if (lyrics?.synced == true) {
                    Pill("−", { scope.launch { graph.settings.setLyricsOffset(track.id, offset - 250) } })
                    Text("%+.2fs".format(offset / 1000f), style = Type.small, color = Color.White, modifier = Modifier.width(52.dp))
                    Pill("+", { scope.launch { graph.settings.setLyricsOffset(track.id, offset + 250) } })
                }
            }
        }
    }
}
