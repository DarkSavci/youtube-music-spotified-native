package dev.darksavci.spotified.ui.screens

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicText
import androidx.compose.foundation.text.TextAutoSize
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import dev.darksavci.spotified.R
import dev.darksavci.spotified.api.AlbumStat
import dev.darksavci.spotified.api.ArtistStat
import dev.darksavci.spotified.api.Artwork
import dev.darksavci.spotified.api.Item
import dev.darksavci.spotified.api.LibraryKind
import dev.darksavci.spotified.api.LookupResults
import dev.darksavci.spotified.api.MonthPlays
import dev.darksavci.spotified.api.SearchFilter
import dev.darksavci.spotified.api.StatDetail
import dev.darksavci.spotified.api.Stats
import dev.darksavci.spotified.api.channelId
import dev.darksavci.spotified.api.cover
import dev.darksavci.spotified.ui.Actions
import dev.darksavci.spotified.ui.LocalActions
import dev.darksavci.spotified.ui.Routes
import dev.darksavci.spotified.ui.components.Art
import dev.darksavci.spotified.ui.components.Chip
import dev.darksavci.spotified.ui.components.EntityRow
import dev.darksavci.spotified.ui.components.GlyphButton
import dev.darksavci.spotified.ui.components.Loaded
import dev.darksavci.spotified.ui.components.Pill
import dev.darksavci.spotified.ui.components.Refreshable
import dev.darksavci.spotified.ui.components.SectionTitle
import dev.darksavci.spotified.ui.components.SkeletonList
import dev.darksavci.spotified.ui.components.TrackRow
import dev.darksavci.spotified.ui.components.rememberLoader
import dev.darksavci.spotified.ui.theme.LocalPalette
import dev.darksavci.spotified.ui.theme.Type
import kotlinx.coroutines.delay

/** The longest span the core's stats will take, in days: as good as all of it. */
private const val ALL_TIME_DAYS = 3650

private val PERIODS = listOf(7 to "7 days", 30 to "30 days", 182 to "6 months", 365 to "A year", ALL_TIME_DAYS to "All time")

@OptIn(ExperimentalLayoutApi::class)
@Composable
fun StatsScreen() {
    val actions = LocalActions.current
    var days by rememberSaveable { mutableIntStateOf(30) }
    var looking by rememberSaveable { mutableStateOf(false) }
    val loader = rememberLoader("stats:$days") { actions.graph.api.stats(days) }
    Column(Modifier.fillMaxSize()) {
        PageTitle("Your listening", actions) {
            GlyphButton(if (looking) R.drawable.ic_x else R.drawable.ic_search, if (looking) "Close the lookup" else "Look up a song or artist", { looking = !looking })
        }
        if (looking) {
            Lookup(actions, Modifier.weight(1f))
        } else {
            // They wrap, so that the last is never cut off at the edge of the screen.
            FlowRow(
                Modifier.fillMaxWidth().padding(horizontal = 16.dp),
                horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                PERIODS.forEach { (d, label) -> Chip(label, days == d, { days = d }) }
            }
            Box(Modifier.weight(1f)) {
                Refreshable(loader) {
                    Loaded(loader, keep = true, skeleton = { SkeletonList() }) { stats -> StatsBody(stats, actions) }
                }
            }
        }
    }
}

/** A row for an artist: it opens their page if they have one, says so if not, and a long press gives the figures. */
@Composable
private fun ArtistRow(a: ArtistStat, actions: Actions) {
    val page = channelId(a.artistId)
    EntityRow(
        a.artist, "${plays(a.plays)} • ${hours(a.totalMs)}", artistPhoto(a.artistId, a.artist) ?: cover(a.artwork),
        { if (page != null) actions.go(Routes.artist(page)) else actions.say("${a.artist} has no page to open: the app wasn't told one when this was played.") },
        round = true,
        onLongClick = { actions.go(Routes.stat("artist", a.artistId.ifEmpty { a.artist })) },
    )
}

@Composable
private fun AlbumRow(a: AlbumStat, actions: Actions) {
    EntityRow(
        a.album, "${a.artist} • ${plays(a.plays)}", cover(a.artwork),
        { if (a.albumId.isNotEmpty()) actions.go(Routes.album(a.albumId)) else actions.say("${a.album} has no page to open: the app wasn't told one when this was played.") },
        onLongClick = { actions.go(Routes.stat("album", a.key.ifEmpty { a.albumId.ifEmpty { a.album } })) },
    )
}

@Composable
private fun StatsBody(stats: Stats, actions: Actions) {
    val p = LocalPalette.current
    LazyColumn(contentPadding = PaddingValues(bottom = 24.dp)) {
        item {
            Row(Modifier.padding(16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Tile("Time", hours(stats.summary.totalMs), Modifier.weight(1f))
                Tile("Plays", stats.summary.plays.toString(), Modifier.weight(1f))
                Tile("Artists", stats.summary.distinctArtists.toString(), Modifier.weight(1f))
            }
        }
        if (stats.summary.plays == 0) item { Text("Nothing played in this time yet.", style = Type.body, color = p.secondary, modifier = Modifier.padding(16.dp)) }
        if (stats.onRepeat.isNotEmpty()) {
            item { SectionTitle("On repeat") }
            val tracks = stats.onRepeat.map { it.track() }
            itemsIndexed(tracks) { i, t -> TrackRow(t, { actions.play(tracks, i, "On repeat") }, { actions.menuFor(t) }, subtitle = plays(stats.onRepeat[i].plays)) }
        }
        if (stats.tracks.isNotEmpty()) {
            item { SectionTitle("Top songs") }
            val top = stats.tracks.map { it.track() }
            itemsIndexed(top.take(20)) { i, t ->
                TrackRow(t, { actions.play(top, i, "Your top songs") }, { actions.menuFor(t) }, number = i + 1, subtitle = "${t.artistNames} • ${plays(stats.tracks[i].plays)}")
            }
        }
        if (stats.artists.isNotEmpty()) {
            item { SectionTitle("Top artists") }
            items(stats.artists) { a -> ArtistRow(a, actions) }
        }
        if (stats.albums.isNotEmpty()) {
            item { SectionTitle("Top albums") }
            items(stats.albums) { a -> AlbumRow(a, actions) }
        }
    }
}

/** A figure in a third of the width: the words shrink to fit, so that a long time is never cut. */
@Composable
internal fun Tile(label: String, value: String, modifier: Modifier) {
    val p = LocalPalette.current
    Column(
        modifier.clip(RoundedCornerShape(8.dp)).background(p.surface).padding(12.dp)
            .semantics(mergeDescendants = true) { contentDescription = "$label: $value" },
    ) {
        BasicText(
            value, style = Type.heading.copy(color = p.text), maxLines = 1,
            autoSize = TextAutoSize.StepBased(minFontSize = 12.sp, maxFontSize = Type.heading.fontSize, stepSize = 1.sp),
        )
        Text(label, style = Type.small, color = p.secondary)
    }
}

// ---------- looking one up ----------

/** Any song, artist or album in the listener's own history, by name; each opens its figures. */
@Composable
private fun Lookup(actions: Actions, modifier: Modifier) {
    val p = LocalPalette.current
    var text by rememberSaveable { mutableStateOf("") }
    val results by produceState<Result<LookupResults>?>(null, text) {
        if (text.isBlank()) {
            value = null
            return@produceState
        }
        delay(250)
        value = runCatching { actions.graph.api.statsLookup(text.trim()) }
    }
    Column(modifier) {
        OutlinedTextField(
            text, { text = it }, Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp),
            placeholder = { Text("A song, artist or album you have played", style = Type.body, color = p.secondary) },
            singleLine = true, textStyle = Type.body,
        )
        val found = results
        LazyColumn(contentPadding = PaddingValues(bottom = 24.dp)) {
            found?.onFailure { item { Line("Couldn't look that up", it.message.orEmpty()) } }
            found?.getOrNull()?.let { r ->
                val tracks = r.tracks.orEmpty()
                val artists = r.artists.orEmpty()
                val albums = r.albums.orEmpty()
                if (tracks.isEmpty() && artists.isEmpty() && albums.isEmpty()) {
                    item { Line("Nothing by that name", "Only what you have played here is counted.") }
                }
                if (artists.isNotEmpty()) item { SectionTitle("Artists") }
                items(artists) { a ->
                    EntityRow(a.artist, plays(a.plays), cover(a.artwork), { actions.go(Routes.stat("artist", a.artistId.ifEmpty { a.artist })) }, round = true)
                }
                if (tracks.isNotEmpty()) item { SectionTitle("Songs") }
                items(tracks) { t ->
                    EntityRow(t.title, "${t.artist} • ${plays(t.plays)}", cover(t.artwork), { actions.go(Routes.stat("track", t.trackId)) })
                }
                if (albums.isNotEmpty()) item { SectionTitle("Albums") }
                items(albums) { a ->
                    EntityRow(a.album, "${a.artist} • ${plays(a.plays)}", cover(a.artwork), { actions.go(Routes.stat("album", a.key.ifEmpty { a.albumId.ifEmpty { a.album } })) })
                }
            }
        }
    }
}

/** The listener's own figures for one song, artist or album. */
@Composable
fun StatDetailScreen(kind: String, id: String) {
    val actions = LocalActions.current
    val loader = rememberLoader("stat:$kind:$id") { actions.graph.api.statDetail(kind, id) }
    Column(Modifier.fillMaxSize()) {
        PageTitle("Your listening", actions)
        Box(Modifier.weight(1f)) {
            Refreshable(loader) {
                Loaded(loader, skeleton = { SkeletonList(rows = 4) }) { detail -> DetailBody(detail, actions) }
            }
        }
    }
}

@Composable
private fun DetailBody(d: StatDetail, actions: Actions) {
    val p = LocalPalette.current
    LazyColumn(contentPadding = PaddingValues(bottom = 24.dp)) {
        item {
            Row(Modifier.padding(16.dp), verticalAlignment = Alignment.CenterVertically) {
                Art(cover(d.artwork), 88.dp)
                Spacer(Modifier.width(16.dp))
                Column(Modifier.weight(1f)) {
                    Text(d.name, style = Type.heading, maxLines = 2, overflow = TextOverflow.Ellipsis)
                    if (d.artist.isNotEmpty()) Text(d.artist, style = Type.body, color = p.secondary)
                }
            }
        }
        item {
            Row(Modifier.padding(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Tile("Plays", d.plays.toString(), Modifier.weight(1f))
                Tile("Time", hours(d.totalMs), Modifier.weight(1f))
                Tile("Rank", if (d.rank > 0) "#${d.rank}" else "–", Modifier.weight(1f))
            }
        }
        item {
            Column(Modifier.padding(16.dp)) {
                Text("${plays(d.plays30d)} in the last 30 days", style = Type.body)
                d.firstPlayedAt?.let { Text("First played ${it.take(10)}", style = Type.small, color = p.secondary) }
                d.lastPlayedAt?.let { Text("Last played ${it.take(10)}", style = Type.small, color = p.secondary) }
            }
        }
        if (d.months.any { it.plays > 0 }) {
            item { SectionTitle("The last twelve months") }
            item { MonthBars(d.months) }
        }
        val page = when (d.kind) {
            "artist" -> channelId(d.id)?.let { Routes.artist(it) }
            "album" -> d.albumId.ifEmpty { null }?.let { Routes.album(it) }
            else -> null
        }
        if (page != null) {
            item { Row(Modifier.padding(16.dp)) { Pill(if (d.kind == "artist") "Open the artist" else "Open the album", { actions.go(page) }) } }
        }
        if (d.topTracks.isNotEmpty()) {
            item { SectionTitle("Most played") }
            val top = d.topTracks.map { it.track() }
            itemsIndexed(top) { i, t -> TrackRow(t, { actions.play(top, i, d.name) }, { actions.menuFor(t) }, subtitle = plays(d.topTracks[i].plays)) }
        }
    }
}

@Composable
private fun MonthBars(months: List<MonthPlays>) {
    val p = LocalPalette.current
    val most = months.maxOf { it.plays }.coerceAtLeast(1)
    Row(
        Modifier.fillMaxWidth().height(96.dp).padding(horizontal = 16.dp)
            .semantics { contentDescription = months.joinToString { "${it.month}: ${plays(it.plays)}" } },
        horizontalArrangement = Arrangement.spacedBy(4.dp), verticalAlignment = Alignment.Bottom,
    ) {
        months.forEach { m ->
            Box(
                Modifier.weight(1f).fillMaxHeight(0.04f + 0.96f * m.plays / most).clip(RoundedCornerShape(2.dp))
                    .background(if (m.plays > 0) p.accent else p.surface),
            )
        }
    }
}

// ---------- artists' pictures ----------

/** Artists' photos, kept for the session: the stats keep a song's cover, not the artist's picture. */
internal val artistPhotos = mutableMapOf<String, List<Artwork>>()

/** [artistId]'s photo from their page, once it has come; null until then or without an id. */
@Composable
internal fun artistPhoto(artistId: String, name: String): List<Artwork>? {
    val id = channelId(artistId) ?: return null
    val api = LocalActions.current.graph.api
    val photo by produceState(artistPhotos[id], id) {
        if (artistPhotos[id] != null) return@produceState
        // The artist's square picture, as the library and search show it; the
        // artist page's own pictures are banners, often with black borders in them.
        val found = runCatching {
            api.library().firstOrNull { it.kind == LibraryKind.Artist && channelId(it.id) == id }?.artwork?.takeIf { it.isNotEmpty() }
                ?: api.search(name, SearchFilter.Artists).shelves.flatMap { it.items }
                    .filterIsInstance<Item.ArtistItem>().firstOrNull { channelId(it.artist.id) == id }?.artwork
        }.getOrNull()
        found?.takeIf { it.isNotEmpty() }?.let { artistPhotos[id] = it; value = it }
    }
    return photo
}
