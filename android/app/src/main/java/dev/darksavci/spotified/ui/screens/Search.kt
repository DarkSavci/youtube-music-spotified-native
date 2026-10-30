package dev.darksavci.spotified.ui.screens

import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.isImeVisible
import dev.darksavci.spotified.ui.components.SearchField
import dev.darksavci.spotified.ui.components.SearchCancel
import androidx.compose.animation.shrinkVertically
import androidx.compose.animation.shrinkHorizontally
import androidx.compose.animation.fadeOut
import androidx.compose.animation.fadeIn
import androidx.compose.animation.expandVertically
import androidx.compose.animation.expandHorizontally
import androidx.compose.animation.core.tween
import androidx.compose.animation.ExitTransition
import androidx.compose.animation.EnterTransition
import androidx.compose.animation.AnimatedVisibility
import androidx.core.graphics.toColorInt
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.asPaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.ime
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.input.pointer.PointerEventPass
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import dev.darksavci.spotified.R
import dev.darksavci.spotified.api.Artwork
import dev.darksavci.spotified.api.Item
import dev.darksavci.spotified.api.MoodChip
import dev.darksavci.spotified.api.SearchFilter
import dev.darksavci.spotified.api.SearchHistoryEntry
import dev.darksavci.spotified.api.SearchResults
import dev.darksavci.spotified.api.Track
import dev.darksavci.spotified.data.Services
import dev.darksavci.spotified.ui.LocalActions
import dev.darksavci.spotified.ui.ProfileButton
import dev.darksavci.spotified.ui.Routes
import dev.darksavci.spotified.ui.components.Art
import dev.darksavci.spotified.ui.components.ChangingBar
import dev.darksavci.spotified.ui.components.Chip
import dev.darksavci.spotified.ui.components.EntityRow
import dev.darksavci.spotified.ui.components.Glyph
import dev.darksavci.spotified.ui.components.GlyphButton
import dev.darksavci.spotified.ui.components.Load
import dev.darksavci.spotified.ui.components.Problem
import dev.darksavci.spotified.ui.components.Refreshable
import dev.darksavci.spotified.ui.components.SectionTitle
import dev.darksavci.spotified.ui.components.SkeletonList
import dev.darksavci.spotified.ui.components.SkeletonTiles
import dev.darksavci.spotified.ui.components.TabScroll
import dev.darksavci.spotified.ui.components.TrackRow
import dev.darksavci.spotified.ui.components.dimmedWhile
import dev.darksavci.spotified.ui.components.keptState
import dev.darksavci.spotified.ui.components.predictiveBack
import dev.darksavci.spotified.ui.components.rememberLoader
import dev.darksavci.spotified.ui.components.subtitleOf
import dev.darksavci.spotified.ui.components.tapRow
import dev.darksavci.spotified.ui.theme.LocalPalette
import dev.darksavci.spotified.ui.theme.Type
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.filter

@OptIn(ExperimentalLayoutApi::class)
@Composable
fun SearchScreen(initial: String = "") {
    val actions = LocalActions.current
    val prefs = PagePrefs.of(LocalContext.current)
    var typed by rememberSaveable { mutableStateOf(initial) }
    var query by rememberSaveable { mutableStateOf(initial) }
    var filter by rememberSaveable { mutableStateOf(SearchFilter.All) }
    var focused by remember { mutableStateOf(false) }
    // Wanted from a keystroke, or from coming back into the field, until results have answered it.
    var wantSuggestions by remember { mutableStateOf(false) }
    val focus = LocalFocusManager.current
    val field = remember { FocusRequester() }

    // The keyboard coming back is a tap back into the field, even though the field never lost focus.
    val keyboard = WindowInsets.isImeVisible
    LaunchedEffect(keyboard) { wantSuggestions = keyboard }

    // Debounced: a query is asked for once typing pauses.
    LaunchedEffect(typed) {
        delay(350)
        query = typed.trim()
    }

    val settle = { text: String ->
        val q = text.trim()
        typed = q
        query = q
        wantSuggestions = false
        focus.clearFocus()
        if (q.isNotEmpty() && !Services.isLink(q)) prefs.rememberSearch(q)
    }
    val cancel = {
        typed = ""
        query = ""
        wantSuggestions = false
        focus.clearFocus()
    }
    predictiveBack(focused || typed.isNotEmpty()) { cancel() }

    Column(Modifier.windowInsetsPadding(WindowInsets.statusBars)) {
        // In use, the field goes to the top and the results get the room.
        val inUse = focused || typed.isNotEmpty()
        val still = actions.reduceMotion
        AnimatedVisibility(
            !inUse,
            enter = if (still) EnterTransition.None else expandVertically(tween(FIELD_MS)) + fadeIn(tween(FIELD_MS)),
            exit = if (still) ExitTransition.None else shrinkVertically(tween(FIELD_MS)) + fadeOut(tween(FIELD_MS / 2)),
        ) {
            Row(Modifier.padding(start = 16.dp, top = 12.dp, bottom = 12.dp), verticalAlignment = Alignment.CenterVertically) {
                ProfileButton(actions, Modifier.padding(end = 12.dp))
                Text("Search", style = Type.title)
            }
        }
        Row(Modifier.padding(start = 16.dp, top = 4.dp, bottom = 8.dp), verticalAlignment = Alignment.CenterVertically) {
            SearchField(
                typed,
                {
                    typed = it
                    wantSuggestions = true
                },
                Modifier.weight(1f),
                focusRequester = field,
                onFocusChanged = {
                    if (it && !focused) wantSuggestions = true
                    focused = it
                },
                onSearch = { settle(typed) },
                onClear = {
                    typed = ""
                    query = ""
                    wantSuggestions = false
                    runCatching { field.requestFocus() }
                },
            )
            // The field gives up its width as the words come in, rather than being cut short by them.
            AnimatedVisibility(
                inUse,
                enter = if (still) EnterTransition.None else expandHorizontally(tween(FIELD_MS)) + fadeIn(tween(FIELD_MS)),
                exit = if (still) ExitTransition.None else shrinkHorizontally(tween(FIELD_MS)) + fadeOut(tween(FIELD_MS / 2)),
            ) {
                SearchCancel(cancel, Modifier.padding(start = 2.dp, end = 4.dp))
            }
            // The gap at the end of the line when there are no words there.
            AnimatedVisibility(
                !inUse,
                enter = if (still) EnterTransition.None else expandHorizontally(tween(FIELD_MS)),
                exit = if (still) ExitTransition.None else shrinkHorizontally(tween(FIELD_MS)),
            ) {
                Spacer(Modifier.width(16.dp))
            }
        }
        when {
            typed.isEmpty() && query.isEmpty() && focused -> RecentSearches(onPick = { settle(it) })
            typed.isEmpty() && query.isEmpty() -> MoodWall()
            Services.isLink(typed.trim()) -> if (Services.isLink(query)) ElsewhereLink(query)
            else -> AnswerList(
                typed.trim(), focused && wantSuggestions, query, filter, { filter = it },
                onAnswered = { wantSuggestions = false }, onPick = { settle(it) }, onTouched = { prefs.rememberSearch(query) },
            )
        }
    }
}

/**
 * What is typed being answered: suggestions while it is, and the results below them.
 * Once the results for what is typed are in, [onAnswered] says the suggestions have done their job.
 */
@Composable
private fun AnswerList(
    typed: String,
    suggesting: Boolean,
    query: String,
    filter: SearchFilter,
    onFilter: (SearchFilter) -> Unit,
    onAnswered: () -> Unit,
    onPick: (String) -> Unit,
    onTouched: () -> Unit,
) {
    val actions = LocalActions.current
    val api = actions.graph.api
    val prefs = PagePrefs.of(LocalContext.current)
    val list = rememberLazyListState()
    TabScroll(Routes.SEARCH, list)
    HidesKeyboard(list)
    // One loader for the page, whatever is asked, so that a search leaves nothing behind per query.
    val ask: suspend () -> SearchResults = { if (query.isEmpty()) SearchResults() else api.search(query, filter) }
    val results = rememberLoader("search:results", fetch = ask)
    results.askFor(query to filter, ask)
    val kept = keptState(results)
    val answered = (kept.state as? Load.Ready)?.value
    val songsListed = remember(answered, prefs.showMusicVideos) { answered?.let { songsListed(it, prefs.showMusicVideos) }.orEmpty() }
    val keys = remember(songsListed) { rowKeys(songsListed) }
    val selection = rememberTrackSelection()
    // A new answer is a new list; and what is chosen from is not under the keyboard.
    LaunchedEffect(query, filter) { selection.stop() }
    val focus = LocalFocusManager.current
    LaunchedEffect(selection.active) { if (selection.active) focus.clearFocus() }
    val answeredNow by rememberUpdatedState(onAnswered)
    // Keyed on the answer and not on the field, so that coming back into the field leaves its suggestions be.
    LaunchedEffect(answered, query, typed, kept.changing) {
        if (answered != null && !kept.changing && query == typed) answeredNow()
    }
    val suggestions by produceState(emptyList<String>(), typed, suggesting) {
        if (!suggesting) {
            value = emptyList()
            return@produceState
        }
        delay(150)
        // A suggestion that does not come costs nothing: what is typed is searched for all the same.
        value = runCatching { api.suggest(typed) }.getOrDefault(value).distinct().take(SUGGESTIONS)
    }
    val touched by rememberUpdatedState(onTouched)

    LazyRow(contentPadding = PaddingValues(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        items(SearchFilter.entries) { f -> Chip(f.label, f == filter, { onFilter(f) }) }
        item {
            Chip(
                "Music videos", prefs.showMusicVideos, { prefs.setVideos(!prefs.showMusicVideos) },
                role = Role.Checkbox, description = if (prefs.showMusicVideos) "Music videos are shown" else "Music videos are hidden",
            )
        }
    }
    ChangingBar(kept.changing)
    Refreshable(results) {
        Box(Modifier.fillMaxSize()) {
        LazyColumn(
            Modifier.fillMaxSize().dimmedWhile(kept.changing)
                // A touch among the results means this was what was looked for; it is worth remembering.
                .pointerInput(Unit) {
                    awaitEachGesture {
                        val down = awaitFirstDown(requireUnconsumed = false, pass = PointerEventPass.Initial)
                        val under = list.layoutInfo.visibleItemsInfo.firstOrNull { down.position.y.toInt() in it.offset until it.offset + it.size }
                        // A suggestion is not what was looked for: what it searches is.
                        if ((under?.key as? String)?.startsWith(SUGGESTION) != true) touched()
                    }
                },
            state = list,
            contentPadding = PaddingValues(bottom = (if (selection.active) 140.dp else 24.dp) + WindowInsets.ime.asPaddingValues().calculateBottomPadding()),
        ) {
            if (suggesting) {
                items(suggestions, key = { SUGGESTION + it }) { text -> SearchLine(R.drawable.ic_search, text, { onPick(text) }) }
            }
            when (val s = kept.state) {
                // Above the keyboard, which covers the page's foot.
                Load.Pending -> item { SkeletonList(rows = 5) }
                is Load.Failed -> item { Problem(s.message, { results.reload() }) }
                is Load.Ready -> when {
                    query.isEmpty() -> Unit
                    // The empty answer to nothing typed yet is not what this search found.
                    kept.changing && s.value.topResult == null && s.value.shelves.isEmpty() -> item { SkeletonList(rows = 5) }
                    else -> results(s.value, prefs.showMusicVideos, keys, selection)
                }
            }
        }
        TrackSelectionBar(selection, songsListed, keys, actions)
        }
    }
}

/** Puts the keyboard away when [list] starts to scroll. */
@Composable
private fun HidesKeyboard(list: LazyListState) {
    val focus = LocalFocusManager.current
    LaunchedEffect(list) {
        snapshotFlow { list.isScrollInProgress }.filter { it }.collect { focus.clearFocus() }
    }
}

/** A line of search text: a recent search or a suggestion, with a way to forget it if it can be. */
@Composable
private fun SearchLine(icon: Int, text: String, onClick: () -> Unit, onForget: (() -> Unit)? = null) {
    val p = LocalPalette.current
    Row(
        Modifier.fillMaxWidth().tapRow(onClick, role = Role.Button, onClickLabel = "Search for this").padding(start = 16.dp, end = 4.dp).height(52.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Glyph(icon, null, tint = p.secondary, size = 20.dp)
        Spacer(Modifier.width(14.dp))
        Text(text, style = Type.body, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f))
        if (onForget != null) GlyphButton(R.drawable.ic_x, "Forget $text", onForget, tint = p.secondary, size = 18.dp)
    }
}

/**
 * The field empty and in use: what was looked for lately, on this phone and on the
 * account, as the desktop lists them. An account's list that does not come leaves this
 * phone's own, which is still the answer.
 */
@Composable
private fun RecentSearches(onPick: (String) -> Unit) {
    val actions = LocalActions.current
    val api = actions.graph.api
    val prefs = PagePrefs.of(LocalContext.current)
    var account by remember { mutableStateOf(emptyList<SearchHistoryEntry>()) }
    LaunchedEffect(Unit) { runCatching { api.searchHistory() }.onSuccess { account = it } }
    val mine = prefs.recentSearches
    val elsewhere = account.filter { e -> mine.none { it.equals(e.query, ignoreCase = true) } }.distinctBy { it.query }
    val list = rememberLazyListState()
    TabScroll(Routes.SEARCH, list)
    HidesKeyboard(list)
    LazyColumn(
        Modifier.fillMaxSize(), state = list,
        contentPadding = PaddingValues(top = 8.dp, bottom = 24.dp + WindowInsets.ime.asPaddingValues().calculateBottomPadding()),
    ) {
        if (mine.isEmpty() && elsewhere.isEmpty()) {
            item { Text("Songs, artists and albums you look for show up here.", style = Type.body, color = LocalPalette.current.secondary, modifier = Modifier.padding(16.dp)) }
        } else {
            item {
                SectionTitle(
                    "Recent searches", action = if (mine.isNotEmpty()) "Clear" else null,
                    onAction = { prefs.clearSearches() },
                )
            }
        }
        items(mine, key = { "mine:$it" }) { q -> SearchLine(R.drawable.ic_clock, q, { onPick(q) }, { prefs.forgetSearch(q) }) }
        items(elsewhere, key = { "account:${it.query}" }) { e ->
            SearchLine(
                R.drawable.ic_clock, e.query, { onPick(e.query) },
                if (e.token.isEmpty()) null else ({
                    account = account - e
                    actions.write { api.forgetSearches(listOf(e.token)) }
                }),
            )
        }
    }
}

/** The rows under the top result. */
private fun topResultRows(results: SearchResults): List<Item> = topResultBeside(results).take(4)

/** Every song the results list, in the order they show, for choosing from. */
private fun songsListed(results: SearchResults, videos: Boolean): List<Track> = buildList {
    results.topResult?.let { top ->
        if (top is Item.Song) add(top.track)
        addAll(topResultRows(results).filterIsInstance<Item.Song>().map { it.track })
    }
    results.shelves.map { shelfShown(it, videos) }.forEach { shelf -> addAll(shelf.items.filterIsInstance<Item.Song>().map { it.track }) }
}

/** The answer to a search, as rows for a list already showing. [keys] are those of [songsListed], in the same order. */
private fun LazyListScope.results(results: SearchResults, videos: Boolean, keys: List<String>, selection: TrackSelection) {
    val shelves = results.shelves.map { shelfShown(it, videos) }.filter { it.items.isNotEmpty() }
    var nextKey = 0
    val keyOf = { item: Item -> if (item is Item.Song) keys[nextKey++] else "" }
    results.topResult?.let { top ->
        val queue = topResultQueue(results)
        val topKey = keyOf(top)
        item { SectionTitle("Top result") }
        item {
            // A song plays on into what is beside it, as the rest of the page's songs do.
            if (top is Item.Song) TrackRowOf(top.track, queue, topKey, selection) else ItemRow(top, artSize = 64.dp)
        }
        val rows = topResultRows(results).map { it to keyOf(it) }
        items(rows) { (item, key) ->
            if (item is Item.Song) TrackRowOf(item.track, queue, key, selection) else ItemRow(item)
        }
    }
    for (shelf in shelves) {
        item { SectionTitle(shelf.title) }
        val songs = shelf.items.filterIsInstance<Item.Song>().map { it.track }
        val rows = shelf.items.map { it to keyOf(it) }
        items(rows) { (item, key) ->
            if (item is Item.Song) {
                TrackRowOf(item.track, songs, key, selection)
            } else {
                ItemRow(item)
            }
        }
    }
    if (results.topResult == null && shelves.isEmpty()) {
        item { Problem("Nothing was found.", null) }
    }
}

/** A song in a shelf of results: it plays on into the shelf's others. */
@Composable
private fun TrackRowOf(track: Track, songs: List<Track>, key: String, selection: TrackSelection) {
    val actions = LocalActions.current
    SelectableRow(track, key, selection, { actions.play(songs, placeInQueue(songs, track), "Search") }, { actions.menuFor(track) })
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
    val list = rememberLazyListState()
    TabScroll(Routes.SEARCH, list)
    HidesKeyboard(list)
    Refreshable(moods) {
        LazyColumn(state = list, contentPadding = PaddingValues(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            item { Text("Browse all", style = Type.heading, modifier = Modifier.padding(bottom = 4.dp)) }
            items(tiles.chunked(2)) { pair ->
                Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                    pair.forEach { mood ->
                        val color = runCatching { Color(mood.color.toColorInt()) }.getOrDefault(LocalPalette.current.surfaceActive)
                        Box(
                            Modifier.weight(1f).height(96.dp).clip(RoundedCornerShape(8.dp)).background(color)
                                .tapRow({ actions.go(Routes.browse(mood.id, mood.params, mood.title)) }, role = Role.Button, onClickLabel = "Open"),
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
            when (val s = state) {
                Load.Pending -> item { SkeletonTiles() }
                is Load.Failed -> item { Problem(s.message, { moods.reload() }) }
                is Load.Ready -> Unit
            }
        }
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
        null -> SkeletonList(rows = 5)
        else -> f.fold(
            { query ->
                Column {
                    Text("Found on YouTube Music as \"$query\"", style = Type.small, color = LocalPalette.current.secondary, modifier = Modifier.padding(16.dp))
                    val ask: suspend () -> SearchResults = { actions.graph.api.search(query) }
                    val results = rememberLoader("search:link", fetch = ask)
                    results.askFor(query, ask)
                    val kept = keptState(results)
                    val prefs = PagePrefs.of(LocalContext.current)
                    val list = rememberLazyListState()
                    TabScroll(Routes.SEARCH, list)
                    HidesKeyboard(list)
                    val answered = (kept.state as? Load.Ready)?.value
                    val songsListed = remember(answered, prefs.showMusicVideos) { answered?.let { songsListed(it, prefs.showMusicVideos) }.orEmpty() }
                    val keys = remember(songsListed) { rowKeys(songsListed) }
                    val selection = rememberTrackSelection()
                    Refreshable(results) {
                        Box(Modifier.fillMaxSize()) {
                            LazyColumn(state = list, contentPadding = PaddingValues(bottom = if (selection.active) 140.dp else 24.dp)) {
                                when (val s = kept.state) {
                                    Load.Pending -> item { SkeletonList(rows = 5) }
                                    is Load.Failed -> item { Problem(s.message, { results.reload() }) }
                                    is Load.Ready -> results(s.value, prefs.showMusicVideos, keys, selection)
                                }
                            }
                            TrackSelectionBar(selection, songsListed, keys, actions)
                        }
                    }
                }
            },
            { Problem(it.message ?: "That link could not be read.", null) },
        )
    }
}

private const val FIELD_MS = 180
private const val SUGGESTIONS = 5
private const val SUGGESTION = "suggest:"
