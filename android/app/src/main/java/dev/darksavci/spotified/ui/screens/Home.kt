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
import dev.darksavci.spotified.api.Account
import dev.darksavci.spotified.api.BrowsePage
import dev.darksavci.spotified.api.Item
import dev.darksavci.spotified.api.Track
import dev.darksavci.spotified.ui.LocalActions
import dev.darksavci.spotified.ui.Routes
import dev.darksavci.spotified.ui.components.Art
import dev.darksavci.spotified.ui.components.Chip
import dev.darksavci.spotified.ui.components.GlyphButton
import dev.darksavci.spotified.ui.components.Load
import dev.darksavci.spotified.ui.components.Loading
import dev.darksavci.spotified.ui.components.Problem
import dev.darksavci.spotified.ui.components.SectionTitle
import dev.darksavci.spotified.ui.components.ShelfRow
import dev.darksavci.spotified.ui.components.TrackRow
import dev.darksavci.spotified.ui.components.rememberLoader
import dev.darksavci.spotified.ui.theme.LocalPalette
import dev.darksavci.spotified.ui.theme.Type
import coil3.compose.AsyncImage

@Composable
fun HomeScreen() {
    val actions = LocalActions.current
    val api = actions.graph.api
    var mood by remember { mutableStateOf("") }
    val page = rememberLoader("home:$mood") { api.home(mood) }
    val recent = rememberLoader("home:recent") { runCatching { api.history() }.getOrDefault(emptyList()) }
    val account by produceState<Account?>(null) { value = runCatching { api.account() }.getOrNull() }
    val state by page.state.collectAsStateWithLifecycle()
    val recentState by recent.state.collectAsStateWithLifecycle()
    val p = LocalPalette.current

    LazyColumn(Modifier.windowInsetsPadding(WindowInsets.statusBars), contentPadding = PaddingValues(bottom = 24.dp)) {
        item {
            Row(Modifier.fillMaxWidth().padding(start = 16.dp, end = 4.dp, top = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                Avatar(account) { actions.go(Routes.SETTINGS) }
                Spacer(Modifier.width(12.dp))
                val chips = (state as? Load.Ready<BrowsePage>)?.value?.chips.orEmpty()
                LazyRow(Modifier.weight(1f), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    item { Chip("All", mood.isEmpty(), { mood = "" }) }
                    items(chips) { chip -> Chip(chip.title, chip.params == mood, { mood = if (mood == chip.params) "" else chip.params }) }
                }
                GlyphButton(R.drawable.ic_clock, "History", { actions.go(Routes.HISTORY) })
                GlyphButton(R.drawable.ic_settings, "Settings", { actions.go(Routes.SETTINGS) })
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
                items(s.value.shelves.filter { it.items.isNotEmpty() }) { shelf ->
                    SectionTitle(
                        shelf.title, action = if (shelf.showAllId.isNotEmpty()) "Show all" else null,
                        onAction = { actions.go(Routes.browse(shelf.showAllId, shelf.showAllParams, shelf.title)) },
                    )
                    // A shelf of songs reads as a list; anything else as cards.
                    val songs = shelf.items.filterIsInstance<Item.Song>()
                    if (songs.size == shelf.items.size && songs.size > 3) {
                        Column {
                            songs.take(4).forEachIndexed { i, song ->
                                TrackRow(song.track, { actions.play(songs.map { it.track }, i, shelf.title) }, { actions.menuFor(song.track) })
                            }
                        }
                    } else {
                        ShelfRow(shelf.items, actions::open, actions::menuFor)
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

@Composable
private fun Avatar(account: Account?, onClick: () -> Unit) {
    val p = LocalPalette.current
    Box(
        Modifier.size(32.dp).clip(CircleShape).background(p.accent).clickable(onClickLabel = "Your account", onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        if (account?.avatarUrl?.isNotEmpty() == true) {
            AsyncImage(account.avatarUrl, null, Modifier.size(32.dp).clip(CircleShape))
        } else {
            Text(account?.name?.take(1)?.uppercase() ?: "?", style = Type.label, color = p.onAccent)
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
