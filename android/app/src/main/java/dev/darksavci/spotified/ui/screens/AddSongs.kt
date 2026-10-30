package dev.darksavci.spotified.ui.screens

import dev.darksavci.spotified.ui.components.SearchField
import androidx.compose.foundation.background
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
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import dev.darksavci.spotified.R
import dev.darksavci.spotified.api.ApiError
import dev.darksavci.spotified.api.Item
import dev.darksavci.spotified.api.SearchFilter
import dev.darksavci.spotified.api.SearchResults
import dev.darksavci.spotified.api.Track
import dev.darksavci.spotified.ui.Change
import dev.darksavci.spotified.ui.LocalActions
import dev.darksavci.spotified.ui.components.ChangingBar
import dev.darksavci.spotified.ui.components.Glyph
import dev.darksavci.spotified.ui.components.GlyphButton
import dev.darksavci.spotified.ui.components.Load
import dev.darksavci.spotified.ui.components.Problem
import dev.darksavci.spotified.ui.components.SectionTitle
import dev.darksavci.spotified.ui.components.SkeletonList
import dev.darksavci.spotified.ui.components.TitleBar
import dev.darksavci.spotified.ui.components.TrackRow
import dev.darksavci.spotified.ui.components.dimmedWhile
import dev.darksavci.spotified.ui.components.keptState
import dev.darksavci.spotified.ui.components.rememberLoader
import dev.darksavci.spotified.ui.theme.LocalPalette
import dev.darksavci.spotified.ui.theme.Type
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.filter

/**
 * Adding songs to one playlist: a search whose rows add rather than play, with
 * what is in the list already ticked, and Back to the playlist. Before anything
 * is typed, what was played lately is offered.
 */
@Composable
fun AddSongsScreen(playlistId: String, title: String) {
    val actions = LocalActions.current
    val api = actions.graph.api
    val p = LocalPalette.current
    var typed by rememberSaveable { mutableStateOf("") }
    var query by rememberSaveable { mutableStateOf("") }
    // Added here, since the playlist's own answer takes a moment to include them.
    var added by rememberSaveable { mutableStateOf(emptyList<String>()) }
    val already = rememberLoader("add:has:$playlistId", follows = setOf(Change.Playlist(playlistId))) { api.completePlaylist(playlistId) }
    val alreadyState by already.state.collectAsStateWithLifecycle()
    val has = (alreadyState as? Load.Ready)?.value?.tracks.orEmpty().mapTo(HashSet()) { it.id } + added
    val recent = rememberLoader("add:recent") { try { api.history() } catch (_: ApiError.SignedOut) { emptyList() } }
    val recentState by recent.state.collectAsStateWithLifecycle()
    val ask: suspend () -> SearchResults = { if (query.isEmpty()) SearchResults() else api.search(query, SearchFilter.Songs) }
    val results = rememberLoader("add:results", fetch = ask)
    results.askFor(query, ask)
    val kept = keptState(results)
    val field = remember { FocusRequester() }
    val focus = LocalFocusManager.current
    val list = rememberLazyListState()

    LaunchedEffect(Unit) { runCatching { field.requestFocus() } }
    LaunchedEffect(typed) {
        delay(350)
        query = typed.trim()
    }
    LaunchedEffect(list) { snapshotFlow { list.isScrollInProgress }.filter { it }.collect { focus.clearFocus() } }

    val add = { t: Track ->
        if (t.id !in has) {
            added = added + t.id
            actions.write(Change.Playlist(playlistId), Change.Library) { api.addToPlaylist(playlistId, listOf(t.id)) }
        }
    }

    @Composable
    fun SongToAdd(t: Track) {
        val isIn = t.id in has
        TrackRow(
            t, { add(t) }, { actions.menuFor(t) },
            playing = false,
            trailing = {
                Box(Modifier.width(48.dp).height(48.dp), contentAlignment = Alignment.Center) {
                    Glyph(if (isIn) R.drawable.ic_check else R.drawable.ic_plus, if (isIn) "In the playlist" else "Add", tint = if (isIn) p.accent else p.text, size = 22.dp)
                }
            },
        )
    }

    Column(Modifier.fillMaxSize()) {
        TitleBar(if (title.isEmpty()) "Add songs" else "Add to $title")
        SearchField(
            typed, { typed = it },
            Modifier.padding(horizontal = 16.dp, vertical = 4.dp).fillMaxWidth(),
            placeholder = "Search for songs", focusRequester = field,
            onClear = { typed = ""; query = "" },
        )
        ChangingBar(kept.changing)
        LazyColumn(
            Modifier.dimmedWhile(kept.changing), state = list,
            contentPadding = PaddingValues(bottom = 24.dp + WindowInsets.ime.asPaddingValues().calculateBottomPadding()),
        ) {
            if (typed.isBlank()) {
                when (val r = recentState) {
                    Load.Pending -> item { SkeletonList(rows = 5) }
                    is Load.Failed -> item { Problem(r.message, { recent.reload() }) }
                    is Load.Ready -> {
                        val songs = r.value.distinctBy { it.id }
                        if (songs.isNotEmpty()) item { SectionTitle("Played lately") }
                        items(songs, key = { "recent:${it.id}" }) { SongToAdd(it) }
                    }
                }
            } else {
                when (val s = kept.state) {
                    Load.Pending -> item { SkeletonList(rows = 5) }
                    is Load.Failed -> item { Problem(s.message, { results.reload() }) }
                    is Load.Ready -> {
                        val songs = s.value.shelves.flatMap { it.items }.filterIsInstance<Item.Song>().map { it.track }.distinctBy { it.id }
                        items(songs, key = { "found:${it.id}" }) { SongToAdd(it) }
                        if (songs.isEmpty() && query.isNotEmpty() && !kept.changing) item { Problem("No songs were found.", null) }
                    }
                }
            }
        }
    }
}
