package dev.darksavci.spotified.ui.screens

import androidx.core.graphics.toColorInt
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
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import dev.darksavci.spotified.R
import dev.darksavci.spotified.api.ArtistRef
import dev.darksavci.spotified.api.Artwork
import dev.darksavci.spotified.api.Item
import dev.darksavci.spotified.api.MoodChip
import dev.darksavci.spotified.api.SearchFilter
import dev.darksavci.spotified.api.SearchResults
import dev.darksavci.spotified.data.Services
import dev.darksavci.spotified.ui.LocalActions
import dev.darksavci.spotified.ui.ProfileButton
import dev.darksavci.spotified.ui.Routes
import dev.darksavci.spotified.ui.components.Art
import dev.darksavci.spotified.ui.components.Chip
import dev.darksavci.spotified.ui.components.EntityRow
import dev.darksavci.spotified.ui.components.Glyph
import dev.darksavci.spotified.ui.components.GlyphButton
import dev.darksavci.spotified.ui.components.Load
import dev.darksavci.spotified.ui.components.Loading
import dev.darksavci.spotified.ui.components.Problem
import dev.darksavci.spotified.ui.components.SectionTitle
import dev.darksavci.spotified.ui.components.TrackRow
import dev.darksavci.spotified.ui.components.rememberLoader
import dev.darksavci.spotified.ui.components.subtitleOf
import dev.darksavci.spotified.ui.theme.LocalPalette
import dev.darksavci.spotified.ui.theme.Type
import kotlinx.coroutines.delay

@Composable
fun SearchScreen(initial: String = "") {
    val actions = LocalActions.current
    val api = actions.graph.api
    val p = LocalPalette.current
    var typed by rememberSaveable { mutableStateOf(initial) }
    var query by rememberSaveable { mutableStateOf(initial) }
    var filter by rememberSaveable { mutableStateOf(SearchFilter.All) }
    val keyboard = LocalSoftwareKeyboardController.current

    // Debounced: a query is asked for once typing pauses.
    LaunchedEffect(typed) {
        delay(350)
        query = typed.trim()
    }

    Column(Modifier.windowInsetsPadding(WindowInsets.statusBars)) {
        Row(Modifier.padding(start = 16.dp, top = 12.dp, bottom = 12.dp), verticalAlignment = Alignment.CenterVertically) {
            ProfileButton(actions, Modifier.padding(end = 12.dp))
            Text("Search", style = Type.title)
        }
        Row(
            Modifier.padding(horizontal = 16.dp).fillMaxWidth().clip(RoundedCornerShape(6.dp))
                .background(if (typed.isEmpty()) Color.White else p.surface).padding(horizontal = 12.dp, vertical = 10.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Glyph(R.drawable.ic_search, null, tint = if (typed.isEmpty()) Color.Black else p.text, size = 22.dp)
            Spacer(Modifier.width(10.dp))
            Box(Modifier.weight(1f)) {
                if (typed.isEmpty()) Text("What do you want to listen to?", style = Type.bodyStrong, color = Color.Black.copy(alpha = 0.7f))
                BasicTextField(
                    typed, { typed = it }, singleLine = true,
                    textStyle = Type.bodyStrong.copy(color = if (typed.isEmpty()) Color.Black else p.text),
                    cursorBrush = SolidColor(p.accent),
                    keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
                    keyboardActions = KeyboardActions(onSearch = { query = typed.trim(); keyboard?.hide() }),
                    modifier = Modifier.fillMaxWidth(),
                )
            }
            if (typed.isNotEmpty()) GlyphButton(R.drawable.ic_x, "Clear", { typed = ""; query = "" }, tint = p.secondary, size = 18.dp)
        }
        if (query.isEmpty()) {
            MoodWall()
            return@Column
        }
        if (Services.isLink(query)) {
            ElsewhereLink(query)
            return@Column
        }
        LazyRow(contentPadding = PaddingValues(16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            items(SearchFilter.entries) { f -> Chip(f.label, f == filter, { filter = f }) }
        }
        val results = rememberLoader("search:$query:$filter") { api.search(query, filter) }
        val state by results.state.collectAsStateWithLifecycle()
        when (val s = state) {
            Load.Pending -> Loading()
            is Load.Failed -> Problem(s.message, { results.reload() })
            is Load.Ready -> Results(s.value)
        }
    }
}

@Composable
private fun Results(results: SearchResults) {
    val actions = LocalActions.current
    LazyColumn(contentPadding = PaddingValues(bottom = 24.dp)) {
        results.topResult?.let { top ->
            item { SectionTitle("Top result") }
            item { ItemRow(top, artSize = 64.dp) }
            // Songs under an artist often come without the artist's name: it is the top result's.
            val by = (top as? Item.ArtistItem)?.artist
            items(results.topResultItems.take(4)) { item ->
                val filled = if (item is Item.Song && item.track.artists.isEmpty() && by != null) {
                    Item.Song(item.track.copy(artists = listOf(ArtistRef(by.id, by.name))))
                } else item
                ItemRow(filled)
            }
        }
        for (shelf in results.shelves.filter { it.items.isNotEmpty() }) {
            item { SectionTitle(shelf.title) }
            val songs = shelf.items.filterIsInstance<Item.Song>().map { it.track }
            items(shelf.items) { item ->
                if (item is Item.Song) {
                    TrackRow(item.track, { actions.play(songs, songs.indexOf(item.track), "Search") }, { actions.menuFor(item.track) })
                } else {
                    ItemRow(item)
                }
            }
        }
        if (results.topResult == null && results.shelves.all { it.items.isEmpty() }) {
            item { Problem("Nothing was found.", null) }
        }
    }
}

@Composable
fun ItemRow(item: Item, artSize: Dp = 48.dp) {
    val actions = LocalActions.current
    when (item) {
        is Item.Song -> TrackRow(item.track, { actions.open(item) }, { actions.menuFor(item) })
        is Item.Episode -> TrackRow(item.track, { actions.open(item) }, { actions.menuFor(item) })
        else -> EntityRow(
            item.title, subtitleOf(item), item.artwork, { actions.open(item) },
            round = item is Item.ArtistItem, onLongClick = { actions.menuFor(item) }, artSize = artSize,
        )
    }
}

/** Before anything is typed: a wall of the moods and genres, each in its colour. */
@Composable
private fun MoodWall() {
    val actions = LocalActions.current
    val moods = rememberLoader("moods") { actions.graph.api.browse("FEmusic_moods_and_genres") }
    val state by moods.state.collectAsStateWithLifecycle()
    val tiles = listOf(
        MoodChip("FEmusic_explore", "", "Discover", "#185a74"),
        MoodChip("FEmusic_new_releases", "", "New releases", "#8c1932"),
        MoodChip("FEmusic_charts", "", "Charts", "#477d95"),
    ) + ((state as? Load.Ready)?.value?.moods.orEmpty())
    LazyColumn(contentPadding = PaddingValues(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        item { Text("Browse all", style = Type.heading, modifier = Modifier.padding(bottom = 4.dp)) }
        items(tiles.chunked(2)) { pair ->
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                pair.forEach { mood ->
                    val color = runCatching { Color(mood.color.toColorInt()) }.getOrDefault(LocalPalette.current.surfaceActive)
                    Box(
                        Modifier.weight(1f).height(96.dp).clip(RoundedCornerShape(8.dp)).background(color)
                            .clickable { actions.go(Routes.browse(mood.id, mood.params, mood.title)) },
                    ) {
                        // A cover from the mood's page, tilted into the corner.
                        val cover = rememberMoodCover(mood)
                        if (cover.isNotEmpty()) {
                            Art(
                                cover, 64.dp,
                                Modifier.align(Alignment.BottomEnd).offset(x = 14.dp, y = 6.dp).rotate(25f).shadow(6.dp),
                            )
                        }
                        // YouTube's own colours run from deep to pale; the words stay readable on both.
                        Text(
                            mood.title, style = Type.heading, maxLines = 2, overflow = TextOverflow.Ellipsis,
                            color = if (color.luminance() > 0.5f) Color.Black else Color.White,
                            modifier = Modifier.padding(12.dp).padding(end = 28.dp),
                        )
                    }
                }
                if (pair.size == 1) Spacer(Modifier.weight(1f))
            }
        }
        if (state is Load.Pending) item { Loading() }
    }
}

/** Covers of the moods' pages, kept for the session: a tile asks once. */
private val moodCovers = mutableMapOf<String, List<Artwork>>()

/** The first cover on [mood]'s page, loaded when its tile shows. */
@Composable
private fun rememberMoodCover(mood: MoodChip): List<Artwork> {
    val api = LocalActions.current.graph.api
    val key = mood.id + "|" + mood.params
    val cover by produceState(moodCovers[key].orEmpty(), key) {
        if (moodCovers[key] != null) return@produceState
        // A cover no other tile has taken, so no two tiles look alike.
        val found = runCatching {
            val covers = api.browse(mood.id, mood.params).shelves.flatMap { it.items }.map { it.artwork }.filter { it.isNotEmpty() }
            // Taken is read once the page is here, after the tiles that answered first chose.
            val taken = moodCovers.values.mapNotNull { it.firstOrNull()?.url }.toSet()
            covers.firstOrNull { it.first().url !in taken } ?: covers.firstOrNull().orEmpty()
        }
        // A failed ask, say while YouTube limits requests, is asked again next time.
        found.onSuccess { moodCovers[key] = it; value = it }
    }
    return cover
}

/** A link from Spotify, Apple Music, Deezer or SoundCloud, matched on YouTube Music. */
@Composable
private fun ElsewhereLink(link: String) {
    val actions = LocalActions.current
    val found by produceState<Result<String>?>(null, link) {
        value = runCatching { Services.read(actions.graph.http, link) }
    }
    when (val f = found) {
        null -> Loading()
        else -> f.fold(
            { query ->
                Column {
                    Text("Found on YouTube Music as \"$query\"", style = Type.small, color = LocalPalette.current.secondary, modifier = Modifier.padding(16.dp))
                    val results = rememberLoader("search:link:$query") { actions.graph.api.search(query) }
                    val state by results.state.collectAsStateWithLifecycle()
                    when (val s = state) {
                        Load.Pending -> Loading()
                        is Load.Failed -> Problem(s.message, { results.reload() })
                        is Load.Ready -> Results(s.value)
                    }
                }
            },
            { Problem(it.message ?: "That link could not be read.", null) },
        )
    }
    @Suppress("UNUSED_VARIABLE") val unused = remember { 0 }
}
