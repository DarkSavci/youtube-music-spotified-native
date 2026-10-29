package dev.darksavci.spotified.ui.screens

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import dev.darksavci.spotified.R
import dev.darksavci.spotified.api.BrowsePage
import dev.darksavci.spotified.api.HomeChip
import dev.darksavci.spotified.api.Item
import dev.darksavci.spotified.api.Track
import dev.darksavci.spotified.ui.LocalActions
import dev.darksavci.spotified.ui.ProfileButton
import dev.darksavci.spotified.ui.Routes
import dev.darksavci.spotified.ui.components.Art
import dev.darksavci.spotified.ui.components.Chip
import dev.darksavci.spotified.ui.components.GlyphButton
import dev.darksavci.spotified.ui.components.Load
import dev.darksavci.spotified.ui.components.Loading
import dev.darksavci.spotified.ui.components.Problem
import dev.darksavci.spotified.ui.components.SectionTitle
import dev.darksavci.spotified.ui.components.ShelfRow
import dev.darksavci.spotified.ui.components.rememberLoader
import dev.darksavci.spotified.ui.theme.LocalPalette
import dev.darksavci.spotified.ui.theme.Type
import androidx.compose.ui.platform.LocalConfiguration
import dev.darksavci.spotified.ui.components.TrackRow
import coil3.compose.AsyncImage

@Composable
fun HomeScreen() {
    val actions = LocalActions.current
    val api = actions.graph.api
    var mood by remember { mutableStateOf("") }
    // Podcasts, as Spotify's Home has them beside All: YouTube Music's own podcasts page.
    // YouTube Music has a podcasts page in some countries only; the chip shows where it does.
    val podcasts by produceState(false) { value = runCatching { api.browse(PODCASTS_PAGE).shelves.isNotEmpty() }.getOrDefault(false) }
    // Home's moods, kept while the podcasts page, which has none, shows.
    var moods by remember { mutableStateOf(emptyList<HomeChip>()) }
    val page = rememberLoader("home:$mood") { if (mood == PODCASTS) api.browse(PODCASTS_PAGE) else api.homeWithMore(mood) }
    val recent = rememberLoader("home:recent") { runCatching { api.history() }.getOrDefault(emptyList()) }
    val state by page.state.collectAsStateWithLifecycle()
    val recentState by recent.state.collectAsStateWithLifecycle()
    val p = LocalPalette.current

    LazyColumn(Modifier.windowInsetsPadding(WindowInsets.statusBars), contentPadding = PaddingValues(bottom = 24.dp)) {
        // Your picture, which opens the side menu, and the moods beside it, as Spotify's Home has them.
        item {
            (state as? Load.Ready<BrowsePage>)?.value?.chips?.takeIf { it.isNotEmpty() }?.let { moods = it }
            val chips = moods
            Row(Modifier.fillMaxWidth().padding(start = 16.dp, top = 12.dp), verticalAlignment = Alignment.CenterVertically) {
                ProfileButton(actions)
                LazyRow(contentPadding = PaddingValues(start = 10.dp, end = 16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    item { Chip("All", mood.isEmpty(), { mood = "" }) }
                    if (podcasts) item { Chip("Podcasts", mood == PODCASTS, { mood = if (mood == PODCASTS) "" else PODCASTS }) }
                    items(chips) { chip -> Chip(chip.title, chip.params == mood, { mood = if (mood == chip.params) "" else chip.params }) }
                }
            }
        }
        val history = (recentState as? Load.Ready<List<Track>>)?.value.orEmpty()
        if (mood.isEmpty() && history.isNotEmpty()) {
            item { RecentGrid(history.distinctBy { it.id }.take(8)) }
        }
        when (val s = state) {
            Load.Pending -> item { Loading() }
            is Load.Failed -> item { Problem(s.message, { page.reload() }) }
            is Load.Ready -> {
                val shelves = s.value.shelves.filter { it.items.isNotEmpty() }
                // One shelf of songs as rows breaks up the covers: Quick picks, as YouTube Music has it, or else the first.
                val songShelves = shelves.filter { sh -> sh.items.size > 4 && sh.items.all { it is Item.Song } }
                val asRows = songShelves.firstOrNull { it.title.equals("Quick picks", ignoreCase = true) } ?: songShelves.firstOrNull()
                items(shelves) { shelf ->
                    SectionTitle(
                        shelf.title, action = if (shelf.showAllId.isNotEmpty()) "Show all" else null,
                        onAction = { actions.go(Routes.browse(shelf.showAllId, shelf.showAllParams, shelf.title)) },
                    )
                    val songs = shelf.items.filterIsInstance<Item.Song>().map { it.track }
                    if (shelf === asRows) {
                        // A shelf of songs reads as rows, four to a column, swiped sideways: YouTube Music's quick picks.
                        SongColumns(songs, shelf.title)
                    } else {
                        // The rest as covers; a song plays its shelf's songs from there.
                        ShelfRow(shelf.items, { item ->
                            if (item is Item.Song) actions.play(songs, songs.indexOf(item.track), shelf.title) else actions.open(item)
                        }, actions::menuFor)
                    }
                }
                if (s.value.shelves.isEmpty()) item { Problem("Nothing here yet.", null) }
            }
        }
        item { Spacer(Modifier.height(16.dp)) }
        item {
            Text("", style = Type.small, color = p.secondary)
        }
    }
}

/** Songs in columns of four rows, each column most of the screen wide so the next one shows at the edge. */
@Composable
private fun SongColumns(songs: List<Track>, origin: String) {
    val actions = LocalActions.current
    val width = LocalConfiguration.current.screenWidthDp.dp * 0.86f
    LazyRow(contentPadding = PaddingValues(end = 16.dp)) {
        items(songs.chunked(4)) { column ->
            Column(Modifier.width(width)) {
                column.forEach { t ->
                    TrackRow(t, { actions.play(songs, songs.indexOf(t), origin) }, { actions.menuFor(t) })
                }
            }
        }
    }
}


/** Two columns of what was played lately, as Spotify's Home opens. */
@Composable
private fun RecentGrid(tracks: List<Track>) {
    val actions = LocalActions.current
    val p = LocalPalette.current
    Column(Modifier.padding(horizontal = 16.dp, vertical = 12.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        tracks.chunked(2).forEach { pair ->
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                pair.forEach { t ->
                    Row(
                        Modifier.weight(1f).height(56.dp).clip(RoundedCornerShape(4.dp)).background(p.surface)
                            .clickable { actions.play(tracks, tracks.indexOf(t), "Recently played") },
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Art(t.artwork, 56.dp, shape = RoundedCornerShape(topStart = 4.dp, bottomStart = 4.dp))
                        Text(
                            t.title, style = Type.label, maxLines = 2, overflow = TextOverflow.Ellipsis,
                            modifier = Modifier.padding(horizontal = 8.dp),
                        )
                    }
                }
                if (pair.size == 1) Spacer(Modifier.weight(1f))
            }
        }
    }
}

/** The chip that turns Home to podcasts; not a mood YouTube Music gives. */
private const val PODCASTS = "podcasts"
private const val PODCASTS_PAGE = "FEmusic_non_music_audio"
