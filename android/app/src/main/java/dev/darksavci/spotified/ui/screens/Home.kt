package dev.darksavci.spotified.ui.screens

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import dev.darksavci.spotified.R
import dev.darksavci.spotified.api.ApiError
import dev.darksavci.spotified.api.HomeChip
import dev.darksavci.spotified.api.Item
import dev.darksavci.spotified.api.Track
import dev.darksavci.spotified.ui.LocalActions
import dev.darksavci.spotified.ui.ProfileButton
import dev.darksavci.spotified.ui.Routes
import dev.darksavci.spotified.ui.components.Art
import dev.darksavci.spotified.ui.components.ChangingBar
import dev.darksavci.spotified.ui.components.Chip
import dev.darksavci.spotified.ui.components.Empty
import dev.darksavci.spotified.ui.components.Glyph
import dev.darksavci.spotified.ui.components.Load
import dev.darksavci.spotified.ui.components.LocalPlayingId
import dev.darksavci.spotified.ui.components.Problem
import dev.darksavci.spotified.ui.components.Refreshable
import dev.darksavci.spotified.ui.components.SectionTitle
import dev.darksavci.spotified.ui.components.ShelfRow
import dev.darksavci.spotified.ui.components.SkeletonShelves
import dev.darksavci.spotified.ui.components.SmallProblem
import dev.darksavci.spotified.ui.components.TabScroll
import dev.darksavci.spotified.ui.components.TrackRow
import dev.darksavci.spotified.ui.components.dimmedWhile
import dev.darksavci.spotified.ui.components.keptState
import dev.darksavci.spotified.ui.components.rememberLoader
import dev.darksavci.spotified.ui.components.tapRow
import dev.darksavci.spotified.ui.theme.LocalPalette
import dev.darksavci.spotified.ui.theme.Type

@Composable
fun HomeScreen() {
    val actions = LocalActions.current
    val api = actions.graph.api
    val prefs = PagePrefs.of(LocalContext.current)
    var mood by rememberSaveable { mutableStateOf("") }
    // Podcasts, as Spotify's Home has them beside All: YouTube Music's own podcasts page, in some
    // countries only. The answer is kept, so that a country without one is not asked on every load.
    var podcasts by remember { mutableStateOf(prefs.podcastsPage() ?: false) }
    LaunchedEffect(Unit) {
        if (prefs.podcastsPage() != null) return@LaunchedEffect
        podcastsAnswer(runCatching { api.browse(PODCASTS_PAGE) })?.let {
            prefs.setPodcastsPage(it)
            podcasts = it
        }
    }
    // Home's moods, kept while the podcasts page, which has none, shows.
    var moods by remember { mutableStateOf(emptyList<HomeChip>()) }
    val page = rememberLoader("home:$mood") { if (mood == PODCASTS) api.browse(PODCASTS_PAGE) else api.homeWithMore(mood) }
    // Being signed out is not a failure here: Home has no recents then, and says so itself.
    val recent = rememberLoader("home:recent") { try { api.history() } catch (_: ApiError.SignedOut) { emptyList() } }
    val kept = keptState(page)
    val recentState by recent.state.collectAsStateWithLifecycle()
    val projection by actions.graph.playback.projection.collectAsStateWithLifecycle()
    val videos = prefs.showMusicVideos
    val list = rememberLazyListState()
    TabScroll(Routes.HOME, list)

    // Another mood starts at its top; coming back to the same one keeps its place.
    var scrolledFor by rememberSaveable { mutableStateOf(mood) }
    LaunchedEffect(mood) {
        if (mood != scrolledFor) {
            scrolledFor = mood
            list.scrollToItem(0)
        }
    }
    LaunchedEffect(kept.state) {
        (kept.state as? Load.Ready)?.let { moods = moodsAfter(moods, it.value, mood.isEmpty()) }
    }

    Refreshable(page, recent) {
        Column(Modifier.windowInsetsPadding(WindowInsets.statusBars)) {
            // Your picture, which opens the side menu, and the moods beside it, as Spotify's Home has them.
            // Above the list, so that they stay where the finger is while a page replaces another.
            Row(Modifier.fillMaxWidth().padding(start = 16.dp, top = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                ProfileButton(actions)
                LazyRow(contentPadding = PaddingValues(start = 10.dp, end = 16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    item { Chip("All", mood.isEmpty(), { mood = "" }) }
                    if (podcasts) item { Chip("Podcasts", mood == PODCASTS, { mood = if (mood == PODCASTS) "" else PODCASTS }) }
                    items(moods) { chip ->
                        Chip(chip.title, chip.params == mood, { mood = if (mood == chip.params) "" else chip.params })
                    }
                }
            }
            ChangingBar(kept.changing)
            LazyColumn(Modifier.dimmedWhile(kept.changing), state = list, contentPadding = PaddingValues(bottom = 24.dp)) {
                if (projection.offline) {
                    item {
                        Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                            Glyph(R.drawable.ic_circle_alert, null, tint = LocalPalette.current.secondary, size = 18.dp)
                            Spacer(Modifier.width(8.dp))
                            Text("You're offline. This is what was kept from last time.", style = Type.small, color = LocalPalette.current.secondary)
                        }
                    }
                }
                if (mood.isEmpty()) {
                    when (val r = recentState) {
                        is Load.Ready -> {
                            val history = songsShown(r.value, videos)
                            if (history.isNotEmpty()) item { RecentGrid(history.distinctBy { it.id }.take(8)) }
                        }
                        // On a lost connection the banner has said it; the core keeps no copy of the history.
                        is Load.Failed -> if (!projection.offline) item { SmallProblem("Couldn't load what you played lately.", { recent.reload() }) }
                        Load.Pending -> Unit
                    }
                }
                when (val s = kept.state) {
                    Load.Pending -> item { SkeletonShelves() }
                    is Load.Failed -> item { Problem(s.message, { page.reload() }) }
                    is Load.Ready -> {
                        val shelves = s.value.shelves.map { shelfShown(it, videos) }.filter { it.items.isNotEmpty() }
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
                        if (shelves.isEmpty()) item { Empty("Nothing here yet" to "This page came back empty. Pull down to try again.") }
                    }
                }
                item { Spacer(Modifier.height(16.dp)) }
            }
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

/** Two columns of what was played lately, as Spotify's Home opens, and the way to the whole list. */
@Composable
private fun RecentGrid(tracks: List<Track>) {
    val actions = LocalActions.current
    val p = LocalPalette.current
    val playingId by LocalPlayingId.current
    Column(Modifier.padding(horizontal = 16.dp).padding(top = 8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        tracks.chunked(2).forEach { pair ->
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                pair.forEach { t ->
                    val current = t.id == playingId
                    Row(
                        Modifier.weight(1f).height(56.dp).clip(RoundedCornerShape(4.dp)).background(p.surface)
                            .tapRow(
                                { actions.play(tracks, tracks.indexOf(t), "Recently played") }, { actions.menuFor(t) },
                                role = Role.Button, onClickLabel = "Play", onLongClickLabel = "More options",
                            )
                            .semantics { if (current) stateDescription = "Now playing" },
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Art(t.artwork, 56.dp, shape = RoundedCornerShape(topStart = 4.dp, bottomStart = 4.dp))
                        Text(
                            t.title, style = Type.label, maxLines = 2, overflow = TextOverflow.Ellipsis,
                            color = if (current) p.accent else p.text,
                            modifier = Modifier.padding(horizontal = 8.dp),
                        )
                    }
                }
                if (pair.size == 1) Spacer(Modifier.weight(1f))
            }
        }
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End) {
            Text(
                "See more", style = Type.label, color = p.secondary,
                modifier = Modifier.clip(RoundedCornerShape(12.dp))
                    .tapRow({ actions.go(Routes.HISTORY) }, role = Role.Button, onClickLabel = "See everything you played lately")
                    .padding(horizontal = 8.dp, vertical = 12.dp),
            )
        }
    }
}

/** The chip that turns Home to podcasts; not a mood YouTube Music gives. */
private const val PODCASTS = "podcasts"
private const val PODCASTS_PAGE = "FEmusic_non_music_audio"
