package dev.darksavci.spotified.ui.screens

import android.Manifest
import android.content.pm.PackageManager
import android.os.Build
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
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
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.core.content.ContextCompat
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import dev.darksavci.spotified.R
import dev.darksavci.spotified.api.Artwork
import dev.darksavci.spotified.api.Stats
import dev.darksavci.spotified.api.channelId
import dev.darksavci.spotified.api.cover
import dev.darksavci.spotified.data.Importer
import dev.darksavci.spotified.data.LocalMusic
import dev.darksavci.spotified.ui.Actions
import dev.darksavci.spotified.ui.LocalActions
import dev.darksavci.spotified.ui.Routes
import dev.darksavci.spotified.ui.components.Art
import dev.darksavci.spotified.ui.components.Chip
import dev.darksavci.spotified.ui.components.EntityRow
import dev.darksavci.spotified.ui.components.GlyphButton
import dev.darksavci.spotified.ui.components.Loaded
import dev.darksavci.spotified.ui.components.Loading
import dev.darksavci.spotified.ui.components.Pill
import dev.darksavci.spotified.ui.components.PlayButton
import dev.darksavci.spotified.ui.components.Problem
import dev.darksavci.spotified.ui.components.SectionTitle
import dev.darksavci.spotified.ui.components.TrackRow
import dev.darksavci.spotified.ui.components.rememberLoader
import dev.darksavci.spotified.ui.theme.LocalPalette
import dev.darksavci.spotified.ui.theme.Type
import kotlinx.coroutines.launch

@Composable
private fun PageTitle(title: String, actions: Actions, trailing: @Composable () -> Unit = {}) {
    Row(Modifier.windowInsetsPadding(WindowInsets.statusBars), verticalAlignment = Alignment.CenterVertically) {
        GlyphButton(R.drawable.ic_chevron_left, "Back", { actions.nav.popBackStack() })
        Text(title, style = Type.title, modifier = Modifier.weight(1f), maxLines = 1, overflow = TextOverflow.Ellipsis)
        trailing()
    }
}

private fun hours(ms: Long): String {
    val minutes = ms / 60_000
    return when {
        minutes >= 60 -> "${minutes / 60} h ${minutes % 60} min"
        minutes == 0L && ms > 0 -> "under a minute"
        else -> "$minutes min"
    }
}

// ---------- your listening ----------

@Composable
fun StatsScreen() {
    val actions = LocalActions.current
    var days by rememberSaveable { mutableIntStateOf(30) }
    val loader = rememberLoader("stats:$days") { actions.graph.api.stats(days) }
    Column {
        PageTitle("Your listening", actions)
        LazyRow(contentPadding = PaddingValues(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            items(listOf(7 to "7 days", 30 to "30 days", 182 to "6 months", 365 to "A year", 0 to "All time")) { (d, label) ->
                Chip(label, days == d, { days = d })
            }
        }
        Loaded(loader) { stats -> StatsBody(stats, actions) }
    }
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
        if (stats.onRepeat.isNotEmpty()) {
            item { SectionTitle("On repeat") }
            val tracks = stats.onRepeat.map { it.track() }
            itemsIndexed(tracks) { i, t -> TrackRow(t, { actions.play(tracks, i, "On repeat") }, { actions.menuFor(t) }, subtitle = plays(stats.onRepeat[i].plays)) }
        }
        item { SectionTitle("Top songs") }
        val top = stats.tracks.map { it.track() }
        itemsIndexed(top.take(20)) { i, t ->
            TrackRow(t, { actions.play(top, i, "Your top songs") }, { actions.menuFor(t) }, number = i + 1, subtitle = "${t.artistNames} • ${plays(stats.tracks[i].plays)}")
        }
        item { SectionTitle("Top artists") }
        items(stats.artists) { a ->
            EntityRow(a.artist, "${plays(a.plays)} • ${hours(a.totalMs)}", artistPhoto(a.artistId) ?: cover(a.artwork), {
                channelId(a.artistId)?.let { actions.go(Routes.artist(it)) }
            }, round = true)
        }
        item { SectionTitle("Top albums") }
        items(stats.albums) { a ->
            EntityRow(a.album, "${a.artist} • ${plays(a.plays)}", cover(a.artwork), {
                if (a.albumId.isNotEmpty()) actions.go(Routes.album(a.albumId))
            })
        }
        if (stats.summary.plays == 0) item { Text("Nothing played in this time yet.", style = Type.body, color = p.secondary, modifier = Modifier.padding(16.dp)) }
    }
}

@Composable
private fun Tile(label: String, value: String, modifier: Modifier) {
    val p = LocalPalette.current
    Column(modifier.clip(RoundedCornerShape(8.dp)).background(p.surface).padding(12.dp)) {
        Text(value, style = Type.heading, maxLines = 1, overflow = TextOverflow.Ellipsis)
        Text(label, style = Type.small, color = p.secondary)
    }
}

/** The year as a few cards to swipe through and share, from the core's own counts. */
@Composable
fun RecapScreen() {
    val actions = LocalActions.current
    val loader = rememberLoader("recap") { actions.graph.api.stats(365) }
    Column(Modifier.fillMaxSize()) {
        PageTitle("Your year", actions)
        Loaded(loader) { stats ->
            if (stats.summary.plays == 0) {
                Problem("Play some music first: the year is made from what you play here.", null)
                return@Loaded
            }
            val cards = buildList<Pair<String, @Composable () -> Unit>> {
                add("You listened for ${hours(stats.summary.totalMs)}" to { Big(hours(stats.summary.totalMs), "of music this year, ${plays(stats.summary.plays)}") })
                stats.tracks.firstOrNull()?.let { t -> add("My song of the year: ${t.title} by ${t.artist}" to { Feature(t.title, t.artist, cover(t.artwork), "Your song of the year, ${plays(t.plays)}") }) }
                stats.artists.firstOrNull()?.let { a -> add("My artist of the year: ${a.artist}" to { Feature(a.artist, "", cover(a.artwork), "Your artist of the year, ${hours(a.totalMs)}") }) }
                add("My top songs: " + stats.tracks.take(5).joinToString("; ") { "${it.title} — ${it.artist}" } to {
                    Column(Modifier.padding(24.dp)) {
                        Text("Your top songs", style = Type.title, color = Color.White)
                        Spacer(Modifier.height(16.dp))
                        stats.tracks.take(5).forEachIndexed { i, t ->
                            Text("${i + 1}  ${t.title}", style = Type.heading, color = Color.White, maxLines = 1, overflow = TextOverflow.Ellipsis)
                            Text("    ${t.artist}", style = Type.body, color = Color.White.copy(alpha = 0.7f), maxLines = 1)
                            Spacer(Modifier.height(8.dp))
                        }
                    }
                })
            }
            val pager = rememberPagerState { cards.size }
            HorizontalPager(pager, Modifier.weight(1f).fillMaxWidth(), contentPadding = PaddingValues(24.dp), pageSpacing = 16.dp) { page ->
                Box(
                    Modifier.fillMaxSize().clip(RoundedCornerShape(16.dp))
                        .background(listOf(Color(0xFF8C1932), Color(0xFF185A74), Color(0xFF477D95), Color(0xFF5A2A82))[page % 4]),
                    contentAlignment = Alignment.Center,
                ) { cards[page].second() }
            }
            Row(Modifier.fillMaxWidth().padding(16.dp), horizontalArrangement = Arrangement.Center) {
                Pill("Share this card", {
                    actions.share("My year in music", cards[pager.currentPage].first + " — via Youtube Music Spotified")
                }, icon = R.drawable.ic_share_2)
            }
        }
    }
}

@Composable
private fun Big(value: String, caption: String) {
    Column(horizontalAlignment = Alignment.CenterHorizontally, modifier = Modifier.padding(24.dp)) {
        Text(value, style = Type.display, color = Color.White)
        Text(caption, style = Type.body, color = Color.White.copy(alpha = 0.8f))
    }
}

@Composable
private fun Feature(title: String, subtitle: String, artwork: List<Artwork>, caption: String) {
    Column(horizontalAlignment = Alignment.CenterHorizontally, modifier = Modifier.padding(24.dp)) {
        Text(caption, style = Type.label, color = Color.White.copy(alpha = 0.8f))
        Spacer(Modifier.height(16.dp))
        Art(artwork, 200.dp)
        Spacer(Modifier.height(16.dp))
        Text(title, style = Type.title, color = Color.White, maxLines = 2, overflow = TextOverflow.Ellipsis)
        if (subtitle.isNotEmpty()) Text(subtitle, style = Type.body, color = Color.White.copy(alpha = 0.8f))
    }
}

// ---------- downloads ----------

@Composable
fun DownloadsScreen() {
    val actions = LocalActions.current
    val downloads = actions.graph.downloads
    val tracks by downloads.tracks.collectAsStateWithLifecycle()
    val done by downloads.done.collectAsStateWithLifecycle()
    val progress by downloads.progress.collectAsStateWithLifecycle()
    val p = LocalPalette.current
    LazyColumn(contentPadding = PaddingValues(bottom = 24.dp)) {
        item {
            PageTitle("Downloads", actions) {
                if (tracks.isNotEmpty()) PlayButton({ actions.play(tracks.filter { it.id in done }, 0, "Downloads") }, size = 44.dp)
                Spacer(Modifier.padding(6.dp))
            }
        }
        item {
            Text(
                "${done.size} of ${tracks.size} songs on this phone." + if (downloads.syncedPlaylists.isNotEmpty()) " ${downloads.syncedPlaylists.size} playlists are kept in step." else "",
                style = Type.small, color = p.secondary, modifier = Modifier.padding(horizontal = 16.dp),
            )
        }
        if (tracks.isEmpty()) item { Problem("Download songs, albums or playlists to play them offline.", null) }
        itemsIndexed(tracks) { i, t ->
            Column {
                TrackRow(t, { actions.play(tracks, i, "Downloads") }, { actions.menuFor(t) }, downloaded = t.id in done)
                progress[t.id]?.let { f ->
                    LinearProgressIndicator({ f }, Modifier.fillMaxWidth().padding(horizontal = 16.dp), color = p.accent, trackColor = p.surface)
                }
            }
        }
    }
}

// ---------- music on this phone ----------

@Composable
fun LocalFilesScreen() {
    val actions = LocalActions.current
    val context = LocalContext.current
    val permission = if (Build.VERSION.SDK_INT >= 33) Manifest.permission.READ_MEDIA_AUDIO else Manifest.permission.READ_EXTERNAL_STORAGE
    var granted by remember { mutableStateOf(ContextCompat.checkSelfPermission(context, permission) == PackageManager.PERMISSION_GRANTED) }
    val ask = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { granted = it }
    Column {
        PageTitle("Music on this phone", actions)
        if (!granted) {
            Problem("Allow the app to read your music files to play them here.", { ask.launch(permission) })
            return@Column
        }
        val loader = rememberLoader("local") { LocalMusic.scan(context) }
        Loaded(loader) { tracks ->
            if (tracks.isEmpty()) {
                Problem("No music files were found on this phone.", null)
                return@Loaded
            }
            LazyColumn(contentPadding = PaddingValues(bottom = 24.dp)) {
                item {
                    Row(Modifier.fillMaxWidth().padding(16.dp), verticalAlignment = Alignment.CenterVertically) {
                        Text("${tracks.size} songs", style = Type.small, color = LocalPalette.current.secondary, modifier = Modifier.weight(1f))
                        GlyphButton(R.drawable.ic_shuffle, "Shuffle", { actions.playback.shuffle(tracks, "On this phone") })
                        PlayButton({ actions.play(tracks, 0, "On this phone") }, size = 44.dp)
                    }
                }
                itemsIndexed(tracks) { i, t -> TrackRow(t, { actions.play(tracks, i, "On this phone") }, { actions.menuFor(t) }) }
            }
        }
    }
}

// ---------- importing ----------

/**
 * A playlist from Spotify, Apple Music or Deezer, by its link or as pasted
 * lines of "Artist - Title": each song is looked for here, the doubtful ones
 * shown for a check, and a playlist made of what was found.
 */
@Composable
fun ImportScreen() {
    val actions = LocalActions.current
    val graph = actions.graph
    val p = LocalPalette.current
    var text by rememberSaveable { mutableStateOf("") }
    var running by remember { mutableStateOf(false) }
    var result by remember { mutableStateOf<Importer.Result?>(null) }
    var name by rememberSaveable { mutableStateOf("") }
    val scope = rememberCoroutineScope()
    LazyColumn(contentPadding = PaddingValues(bottom = 24.dp)) {
        item { PageTitle("Import a playlist", actions) }
        item {
            Text(
                "Paste a playlist's link from Spotify, Apple Music or Deezer, or its songs one per line as \"Artist - Title\" (as exported from those services).",
                style = Type.small, color = p.secondary, modifier = Modifier.padding(horizontal = 16.dp),
            )
            OutlinedTextField(text, { text = it }, Modifier.fillMaxWidth().padding(16.dp).height(140.dp), label = { Text("Link or songs") })
            Row(Modifier.padding(horizontal = 16.dp)) {
                Pill(if (running) "Looking…" else "Find the songs", {
                    if (running || text.isBlank()) return@Pill
                    running = true
                    scope.launch {
                        result = runCatching { Importer.match(graph.http, graph.api, text) }.onFailure { actions.say(it.message ?: "That could not be read.") }.getOrNull()
                        name = result?.name.orEmpty()
                        running = false
                    }
                })
            }
        }
        if (running) item { Loading() }
        result?.let { r ->
            item {
                Text("${r.found.size} of ${r.wanted} found" + if (r.doubtful.isNotEmpty()) "; ${r.doubtful.size} to check" else "", style = Type.bodyStrong, modifier = Modifier.padding(16.dp))
                OutlinedTextField(name, { name = it }, Modifier.fillMaxWidth().padding(horizontal = 16.dp), label = { Text("Playlist name") }, singleLine = true)
                Row(Modifier.padding(16.dp)) {
                    Pill("Make the playlist", {
                        actions.launch {
                            val id = graph.api.createPlaylist(name.ifBlank { "Imported" })
                            r.found.map { it.id }.chunked(50).forEach { graph.api.addToPlaylist(id, it) }
                            actions.say("Made ${name.ifBlank { "Imported" }} with ${r.found.size} songs")
                            actions.go(Routes.playlist(id))
                        }
                    }, icon = R.drawable.ic_check)
                }
            }
            if (r.doubtful.isNotEmpty()) item { SectionTitle("Check these") }
            items(r.doubtful) { (wanted, match) ->
                Column {
                    Text("Wanted: $wanted", style = Type.small, color = p.secondary, modifier = Modifier.padding(horizontal = 16.dp))
                    TrackRow(match, { actions.play(listOf(match), 0, "Import") }, { actions.menuFor(match) })
                }
            }
            if (r.missing.isNotEmpty()) {
                item { SectionTitle("Not found") }
                items(r.missing) { Text(it, style = Type.body, color = p.secondary, modifier = Modifier.padding(horizontal = 16.dp, vertical = 4.dp)) }
            }
        }
    }
}

private fun plays(n: Int) = if (n == 1) "1 play" else "$n plays"

/** Artists' photos, kept for the session: the stats keep a song's cover, not the artist's picture. */
private val artistPhotos = mutableMapOf<String, List<Artwork>>()

/** [artistId]'s photo from their page, once it has come; null until then or without an id. */
@Composable
private fun artistPhoto(artistId: String): List<Artwork>? {
    val id = channelId(artistId) ?: return null
    val api = LocalActions.current.graph.api
    val photo by produceState(artistPhotos[id], id) {
        if (artistPhotos[id] != null) return@produceState
        runCatching { api.artist(id).artwork }.getOrNull()?.takeIf { it.isNotEmpty() }?.let { artistPhotos[id] = it; value = it }
    }
    return photo
}
