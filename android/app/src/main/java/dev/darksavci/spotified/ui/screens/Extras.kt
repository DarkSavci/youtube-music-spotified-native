package dev.darksavci.spotified.ui.screens

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.os.SystemClock
import android.provider.OpenableColumns
import android.provider.Settings as SystemSettings
import androidx.activity.compose.LocalActivity
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
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
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.core.content.ContextCompat
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LifecycleEventEffect
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import dev.darksavci.spotified.R
import dev.darksavci.spotified.api.Track
import dev.darksavci.spotified.data.DownloadGroup
import dev.darksavci.spotified.data.DownloadStatus
import dev.darksavci.spotified.data.ExportFile
import dev.darksavci.spotified.data.Importer
import dev.darksavci.spotified.data.LocalMusic
import dev.darksavci.spotified.data.groupByAlbum
import dev.darksavci.spotified.data.sizeText
import dev.darksavci.spotified.ui.Actions
import dev.darksavci.spotified.ui.Change
import dev.darksavci.spotified.ui.LocalActions
import dev.darksavci.spotified.ui.Routes
import dev.darksavci.spotified.ui.components.Art
import dev.darksavci.spotified.ui.components.Chip
import dev.darksavci.spotified.ui.components.Empty
import dev.darksavci.spotified.ui.components.GlyphButton
import dev.darksavci.spotified.ui.components.Load
import dev.darksavci.spotified.ui.components.Loaded
import dev.darksavci.spotified.ui.components.Pill
import dev.darksavci.spotified.ui.components.PlayButton
import dev.darksavci.spotified.ui.components.Problem
import dev.darksavci.spotified.ui.components.Refreshable
import dev.darksavci.spotified.ui.components.SectionTitle
import dev.darksavci.spotified.ui.components.SkeletonList
import dev.darksavci.spotified.ui.components.TrackRow
import dev.darksavci.spotified.ui.components.rememberLoader
import dev.darksavci.spotified.ui.theme.LocalPalette
import dev.darksavci.spotified.ui.theme.Type
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

@Composable
internal fun PageTitle(title: String, actions: Actions, trailing: @Composable () -> Unit = {}) {
    Row(Modifier.windowInsetsPadding(WindowInsets.statusBars), verticalAlignment = Alignment.CenterVertically) {
        GlyphButton(R.drawable.ic_chevron_left, "Back", { actions.nav.popBackStack() })
        Text(title, style = Type.title, modifier = Modifier.weight(1f).semantics { heading() }, maxLines = 1, overflow = TextOverflow.Ellipsis)
        trailing()
    }
}

// ---------- downloads ----------

@Composable
fun DownloadsScreen() {
    val actions = LocalActions.current
    val downloads = actions.graph.downloads
    val tracks by downloads.tracks.collectAsStateWithLifecycle()
    val done by downloads.done.collectAsStateWithLifecycle()
    val statuses by downloads.statuses.collectAsStateWithLifecycle()
    val paused by downloads.paused.collectAsStateWithLifecycle()
    val bytes by downloads.bytes.collectAsStateWithLifecycle()
    val waitingForWifi by downloads.waitingForWifi.collectAsStateWithLifecycle()
    val unpulled by downloads.unpulled.collectAsStateWithLifecycle()
    val p = LocalPalette.current
    var byAlbum by rememberSaveable { mutableStateOf(false) }
    val loader = rememberLoader("downloads") { downloads.refresh() }
    val state by loader.state.collectAsStateWithLifecycle()
    val finished = tracks.filter { it.id in done }
    val coming = tracks.filter { it.id !in done }
    val failed = coming.any { statuses[it.id] is DownloadStatus.Failed }
    Column(Modifier.fillMaxSize()) {
        PageTitle("Downloads", actions) {
            if (finished.isNotEmpty()) {
                GlyphButton(R.drawable.ic_shuffle, "Shuffle downloads", { actions.playback.shuffle(finished, "Downloads") })
                PlayButton({ actions.play(finished, 0, "Downloads") }, size = 44.dp)
                Spacer(Modifier.width(12.dp))
            }
        }
        Box(Modifier.weight(1f)) {
            Refreshable(loader) {
                LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 24.dp)) {
                    item {
                        val kept = downloads.syncedPlaylists.size
                        val line = listOfNotNull(
                            "${finished.size} of ${tracks.size} songs on this phone".takeIf { tracks.isNotEmpty() },
                            sizeText(bytes).takeIf { tracks.isNotEmpty() },
                            (if (kept == 1) "1 playlist kept in step" else "$kept playlists kept in step").takeIf { kept > 0 },
                        ).joinToString(" • ")
                        if (line.isNotEmpty()) Text(line, style = Type.small, color = p.secondary, modifier = Modifier.padding(horizontal = 16.dp))
                        if (waitingForWifi && !paused && (coming.isNotEmpty() || unpulled.isNotEmpty())) {
                            Text("Waiting for Wi-Fi. Download on Wi-Fi only is on in Settings.", style = Type.small, color = p.secondary, modifier = Modifier.padding(horizontal = 16.dp, vertical = 4.dp))
                        }
                    }
                    if (state is Load.Failed) {
                        item { Text("Couldn't ask how the downloads are getting on. Pull down to try again.", style = Type.small, color = p.danger, modifier = Modifier.padding(16.dp)) }
                    }
                    if (tracks.isEmpty()) item { Empty("No downloads yet" to "Tap the download button on a song, album or playlist to play it offline.") }
                    if (coming.isNotEmpty()) {
                        item {
                            Text(if (paused) "Paused" else if (waitingForWifi) "Waiting for Wi-Fi" else "Under way", style = Type.heading, modifier = Modifier.padding(start = 16.dp, end = 16.dp, top = 16.dp, bottom = 4.dp).semantics { heading() })
                            if (paused) Text("Stopped for now. Resuming carries on from where each song got to.", style = Type.small, color = p.secondary, modifier = Modifier.padding(horizontal = 16.dp))
                            Row(Modifier.padding(horizontal = 16.dp, vertical = 8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                                if (paused) Pill("Resume", { downloads.resume() }, icon = R.drawable.ic_play)
                                else Pill("Pause", { downloads.pause() }, icon = R.drawable.ic_pause)
                                if (failed) Pill("Retry failed", { downloads.retryAll() }, icon = R.drawable.ic_rotate_ccw)
                                Pill("Cancel all", {
                                    actions.confirm(
                                        if (coming.size == 1) "Cancel this download?" else "Cancel ${coming.size} downloads?",
                                        "The songs that have finished stay on the phone.", "Stop downloading",
                                    ) { downloads.remove(coming) }
                                }, icon = R.drawable.ic_x)
                            }
                        }
                        items(coming, key = { "coming:" + it.id }) { t -> ComingRow(t, statuses[t.id], actions, waitingForWifi && !paused) }
                    }
                    if (finished.isNotEmpty()) {
                        item {
                            Row(Modifier.padding(horizontal = 16.dp, vertical = 12.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                                Chip("Songs", !byAlbum, { byAlbum = false })
                                Chip("Albums", byAlbum, { byAlbum = true })
                            }
                        }
                        if (byAlbum) {
                            groupByAlbum(finished).forEach { group ->
                                item(key = "group:" + group.key) { GroupHeader(group, actions) }
                                items(group.tracks, key = { "song:" + group.key + it.id }) { t ->
                                    TrackRow(t, { actions.play(group.tracks, group.tracks.indexOf(t), group.title) }, { actions.menuFor(t) }, downloaded = true, showArt = false)
                                }
                            }
                        } else {
                            itemsIndexed(finished, key = { _, t -> "song:" + t.id }) { i, t ->
                                TrackRow(t, { actions.play(finished, i, "Downloads") }, { actions.menuFor(t) }, downloaded = true)
                            }
                        }
                    }
                }
            }
        }
    }
}

/** A song not whole yet: it can't be played from here, so its row says how it is getting on and offers what can be done. */
@Composable
private fun ComingRow(t: Track, status: DownloadStatus?, actions: Actions, waitingForWifi: Boolean) {
    val p = LocalPalette.current
    val downloads = actions.graph.downloads
    val (line, fraction) = when (status) {
        is DownloadStatus.Active -> (status.fraction?.let { "${(it * 100).toInt()}%" } ?: "Starting…") to status.fraction
        is DownloadStatus.Waiting -> "Waiting" to status.fraction
        is DownloadStatus.Paused -> (if (waitingForWifi) "Waiting for Wi-Fi" else "Paused") to status.fraction
        is DownloadStatus.Failed -> "Didn't finish" to status.fraction
        else -> "Waiting" to null
    }
    Column {
        TrackRow(
            t, { actions.say("This song is still downloading. It can be played when it has finished.") }, { actions.menuFor(t) },
            modifier = Modifier.alpha(0.75f), subtitle = line, playing = false,
            trailing = {
                if (status is DownloadStatus.Failed) GlyphButton(R.drawable.ic_rotate_ccw, "Retry ${t.title}", { downloads.retry(t.id) }, tint = p.accent, size = 20.dp)
                GlyphButton(R.drawable.ic_x, "Cancel ${t.title}", { downloads.remove(listOf(t)) }, tint = p.secondary, size = 20.dp)
            },
        )
        if (fraction != null && status !is DownloadStatus.Failed) {
            LinearProgressIndicator({ fraction }, Modifier.fillMaxWidth().padding(horizontal = 16.dp), color = p.accent, trackColor = p.surface)
        }
    }
}

@Composable
private fun GroupHeader(group: DownloadGroup, actions: Actions) {
    val p = LocalPalette.current
    Row(Modifier.fillMaxWidth().padding(start = 16.dp, end = 4.dp, top = 16.dp, bottom = 4.dp), verticalAlignment = Alignment.CenterVertically) {
        Art(group.artwork, 48.dp, shape = RoundedCornerShape(4.dp))
        Spacer(Modifier.width(12.dp))
        Column(Modifier.weight(1f).semantics(mergeDescendants = true) { heading() }) {
            Text(group.title, style = Type.bodyStrong, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text(
                listOf(group.artist, if (group.tracks.size == 1) "1 song" else "${group.tracks.size} songs").filter { it.isNotEmpty() }.joinToString(" • "),
                style = Type.small, color = p.secondary, maxLines = 1, overflow = TextOverflow.Ellipsis,
            )
        }
        GlyphButton(R.drawable.ic_trash_2, "Remove ${group.title} from downloads", {
            actions.confirm("Remove ${group.title} from downloads?", "Its songs go from this phone, and can be downloaded again.", "Remove") {
                actions.graph.downloads.remove(group.tracks)
            }
        }, tint = p.secondary, size = 20.dp)
    }
}

// ---------- music on this phone ----------

/** An answer quicker than this came from Android alone: nobody saw the question. */
internal const val NO_QUESTION_MS = 400L

/**
 * Whether only Settings can give the phone's music now. Android will not explain
 * before the first question either, nor after one dismissed without an answer, so
 * "won't explain" alone says nothing: it counts after a question it would have
 * explained, or after an answer that came with no question shown.
 */
internal fun permanentlyRefused(granted: Boolean, couldExplainBefore: Boolean, canExplainNow: Boolean, answeredInMs: Long) =
    !granted && !canExplainNow && (couldExplainBefore || answeredInMs < NO_QUESTION_MS)

@Composable
fun LocalFilesScreen() {
    val actions = LocalActions.current
    val context = LocalContext.current
    val activity = LocalActivity.current
    val permission = if (Build.VERSION.SDK_INT >= 33) Manifest.permission.READ_MEDIA_AUDIO else Manifest.permission.READ_EXTERNAL_STORAGE
    fun has() = ContextCompat.checkSelfPermission(context, permission) == PackageManager.PERMISSION_GRANTED
    var granted by remember { mutableStateOf(has()) }
    var blocked by rememberSaveable { mutableStateOf(false) }
    fun canExplain() = activity?.shouldShowRequestPermissionRationale(permission) == true
    // Taken at the answer itself: a refusal like the last changes no state, so nothing would look again later.
    var couldExplain by remember { mutableStateOf(false) }
    var askedAt by remember { mutableLongStateOf(0L) }
    val ask = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) {
        granted = it
        blocked = permanentlyRefused(it, couldExplain, canExplain(), SystemClock.elapsedRealtime() - askedAt)
    }
    // Coming back from the system's page, the answer may have changed.
    LifecycleEventEffect(Lifecycle.Event.ON_RESUME) { granted = has() }
    Column(Modifier.fillMaxSize()) {
        PageTitle("Music on this phone", actions)
        if (!granted) {
            if (blocked) {
                Problem("Android won't ask again, so reading your music files has to be allowed in the system's settings for this app, under Permissions.", null)
                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.Center) {
                    Pill("Open settings", {
                        // The answer may change there; asking again finds out at once if it did not.
                        blocked = false
                        val page = Intent(SystemSettings.ACTION_APPLICATION_DETAILS_SETTINGS, Uri.parse("package:${context.packageName}"))
                        runCatching { context.startActivity(page) }.onFailure { actions.say("The system's settings could not be opened.") }
                    }, primary = true)
                }
            } else {
                Problem("Allow the app to read your music files to play them here.", null)
                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.Center) {
                    Pill("Allow", {
                        couldExplain = canExplain()
                        askedAt = SystemClock.elapsedRealtime()
                        ask.launch(permission)
                    }, primary = true)
                }
            }
            return@Column
        }
        val loader = rememberLoader("local") { LocalMusic.scan(context) }
        Box(Modifier.weight(1f)) {
            Refreshable(loader) {
                Loaded(loader, skeleton = { SkeletonList() }) { tracks ->
                    LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 24.dp)) {
                        if (tracks.isEmpty()) {
                            item { Empty("No music files" to "Songs you copy onto this phone show up here.") }
                            return@LazyColumn
                        }
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
    }
}

// ---------- importing ----------

/**
 * A playlist from Spotify, Apple Music or Deezer: by its link, as pasted lines
 * of "Artist - Title", or from a file they exported. Each song is looked for
 * here, the doubtful ones shown for a check, and a playlist made of what was found.
 */
@Composable
fun ImportScreen() {
    val actions = LocalActions.current
    val graph = actions.graph
    val context = LocalContext.current
    val p = LocalPalette.current
    var text by rememberSaveable { mutableStateOf("") }
    var running by remember { mutableStateOf(false) }
    var making by remember { mutableStateOf(false) }
    var looked by remember { mutableStateOf<Pair<Int, Int>?>(null) }
    var result by remember { mutableStateOf<Importer.Result?>(null) }
    var name by rememberSaveable { mutableStateOf("") }
    val scope = rememberCoroutineScope()

    /** Runs a search of the songs, showing how far it has got and saying a failure in words. */
    fun search(find: suspend ((Int, Int) -> Unit) -> Importer.Result) {
        if (running) return
        running = true
        result = null
        looked = null
        scope.launch {
            result = runCatching { find { done, of -> looked = done to of } }
                .onFailure { actions.say(it.message ?: "That could not be read.") }.getOrNull()
            name = result?.name.orEmpty()
            running = false
        }
    }

    val file = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri: Uri? ->
        if (uri == null) return@rememberLauncherForActivityResult
        search { progress ->
            val (fileName, songs) = withContext(Dispatchers.IO) {
                val bytes = readLimited(context, uri, ExportFile.MOST_BYTES) ?: error("That file is too large to be a playlist.")
                val title = context.contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { c ->
                    if (c.moveToFirst()) c.getString(0) else null
                }.orEmpty()
                ExportFile.read(title, ExportFile.decode(bytes))
            }
            Importer.matchSongs(graph.api, fileName, songs, progress)
        }
    }

    Column(Modifier.fillMaxSize()) {
        PageTitle("Import a playlist", actions)
        LazyColumn(Modifier.weight(1f), contentPadding = PaddingValues(bottom = 24.dp)) {
            item {
                Text(
                    "Paste a playlist's link from Spotify, Apple Music or Deezer, or its songs one per line as \"Artist - Title\". Or choose a file those services or Exportify exported: CSV, TSV, Spotify's JSON or M3U.",
                    style = Type.small, color = p.secondary, modifier = Modifier.padding(horizontal = 16.dp),
                )
                OutlinedTextField(text, { text = it }, Modifier.fillMaxWidth().padding(16.dp).height(140.dp), label = { Text("Link or songs") })
                Row(Modifier.padding(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                    Pill(primary = true, text = if (running) "Looking…" else "Find the songs", enabled = !running && text.isNotBlank(), onClick = {
                        val input = text
                        search { progress -> Importer.match(graph.http, graph.api, input, progress) }
                    })
                    Pill("Choose a file", { if (!running) file.launch(arrayOf("*/*")) }, icon = R.drawable.ic_folder, enabled = !running)
                }
            }
            if (running) {
                item {
                    val at = looked
                    Column(Modifier.fillMaxWidth().padding(16.dp)) {
                        Text(if (at == null) "Reading the playlist…" else "Looked for ${at.first} of ${at.second} songs", style = Type.small, color = p.secondary)
                        Spacer(Modifier.height(8.dp))
                        if (at == null) LinearProgressIndicator(Modifier.fillMaxWidth(), color = p.accent, trackColor = p.surface)
                        else LinearProgressIndicator({ at.first.toFloat() / at.second }, Modifier.fillMaxWidth(), color = p.accent, trackColor = p.surface)
                    }
                }
            }
            result?.let { r ->
                item {
                    Text("${r.found.size} of ${r.wanted} found" + if (r.doubtful.isNotEmpty()) "; ${r.doubtful.size} to check" else "", style = Type.bodyStrong, modifier = Modifier.padding(16.dp))
                    OutlinedTextField(name, { name = it }, Modifier.fillMaxWidth().padding(horizontal = 16.dp), label = { Text("Playlist name") }, singleLine = true)
                    Row(Modifier.padding(16.dp)) {
                        Pill(if (making) "Making it…" else "Make the playlist", {
                            if (making) return@Pill
                            making = true
                            // On this page's own scope, which is the main thread: it ends by opening a page.
                            scope.launch {
                                runCatching {
                                    val title = name.ifBlank { "Imported" }
                                    val id = graph.api.createPlaylist(title)
                                    r.found.map { it.id }.chunked(50).forEach { graph.api.addToPlaylist(id, it) }
                                    actions.changed(Change.Library)
                                    actions.say("Made $title with ${r.found.size} songs")
                                    actions.go(Routes.playlist(id))
                                }.onFailure { actions.say(it.message ?: "The playlist could not be made.") }
                                making = false
                            }
                        }, icon = R.drawable.ic_check, enabled = !making && r.found.isNotEmpty())
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
}
