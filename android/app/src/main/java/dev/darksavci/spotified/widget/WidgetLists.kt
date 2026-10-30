package dev.darksavci.spotified.widget

import android.content.Context
import android.graphics.Bitmap
import android.os.SystemClock
import androidx.compose.runtime.Composable
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.glance.GlanceId
import androidx.glance.GlanceModifier
import androidx.glance.Image
import androidx.glance.ImageProvider
import androidx.glance.action.Action
import androidx.glance.action.ActionParameters
import androidx.glance.action.actionParametersOf
import androidx.glance.action.clickable
import androidx.glance.appwidget.action.ActionCallback
import androidx.glance.appwidget.action.actionRunCallback
import androidx.glance.appwidget.cornerRadius
import androidx.glance.appwidget.lazy.LazyColumn
import androidx.glance.appwidget.lazy.items
import androidx.glance.appwidget.updateAll
import androidx.glance.background
import androidx.glance.ColorFilter
import androidx.glance.layout.Alignment
import androidx.glance.layout.Column
import androidx.glance.layout.Row
import androidx.glance.layout.Spacer
import androidx.glance.layout.fillMaxSize
import androidx.glance.layout.fillMaxWidth
import androidx.glance.layout.padding
import androidx.glance.layout.size
import androidx.glance.layout.width
import androidx.glance.text.FontWeight
import androidx.glance.text.Text
import androidx.glance.text.TextStyle
import dev.darksavci.spotified.R
import dev.darksavci.spotified.api.ApiError
import dev.darksavci.spotified.api.Track
import dev.darksavci.spotified.graph
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeoutOrNull

/** A list of songs as a widget shows it. */
internal data class WidgetSongs(
    val title: String = "",
    val tracks: List<Track> = emptyList(),
    val art: Map<String, Bitmap> = emptyMap(),
    /** Why the last load failed; what was shown before stays. */
    val failed: String? = null,
    val loading: Boolean = false,
    /** When it was last loaded, on the clock that only runs forward; null if never. */
    val loadedAt: Long? = null,
)

/**
 * The song to play from in the list a row came from. By its id, as the list can
 * have changed since the row was drawn; null if the song has left it.
 */
internal fun startOf(tracks: List<Track>, song: String): Int? =
    tracks.indexOfFirst { it.id == song }.takeIf { it >= 0 }

/** The rows a list shows: what can be played, once each, as a row is known by its song. */
internal fun rowsOf(tracks: List<Track>): List<Track> =
    tracks.filter { it.playable && it.id.isNotEmpty() }.distinctBy { it.id }.take(WidgetLists.ROWS)

/**
 * The lists the widgets show, by key, for the life of the process. A press plays
 * from what was drawn, without fetching it again, and widgets of one playlist
 * share its list.
 */
internal object WidgetLists {
    const val RECENT = "recent"
    const val ROWS = 15

    /** How old a list may be when a widget is drawn again by itself. */
    private const val STALE_MS = 5 * 60_000L

    /** How long a press waits for a list it does not have, before playing the song alone. */
    private const val PRESS_WAIT_MS = 3000L

    /** The size of a row's cover, in pixels. */
    private const val ROW_ART_PX = 96

    fun playlistKey(id: String) = "playlist:$id"

    private val lists = MutableStateFlow<Map<String, WidgetSongs>>(emptyMap())

    fun of(key: String): Flow<WidgetSongs> = lists.map { it[key] ?: WidgetSongs() }.distinctUntilChanged()

    fun now(key: String): WidgetSongs = lists.value[key] ?: WidgetSongs()

    /** Loads the list if it is missing or has grown old. */
    suspend fun freshen(context: Context, key: String) {
        val at = now(key).loadedAt
        if (at == null || SystemClock.elapsedRealtime() - at > STALE_MS) refresh(context, key)
    }

    /** Loads the list again, leaving what is shown in place if that fails. */
    suspend fun refresh(context: Context, key: String) {
        if (now(key).loading) return
        change(key) { it.copy(loading = true, failed = null) }
        val graph = context.graph
        runCatching {
            graph.core.running()
            if (key == RECENT) {
                "Recently played" to graph.api.history()
            } else {
                graph.api.completePlaylist(key.removePrefix("playlist:")).let { it.title to it.tracks }
            }
        }.fold(
            { (title, tracks) ->
                val art = rowsOf(tracks).mapNotNull { t -> widgetBitmap(context, t.artwork, ROW_ART_PX, small = true)?.let { t.id to it } }.toMap()
                change(key) { WidgetSongs(title, tracks, art, null, false, SystemClock.elapsedRealtime()) }
            },
            { e ->
                val why = if (e is ApiError.SignedOut) "Sign in to see this." else "Couldn’t load this. Try refresh."
                change(key) { it.copy(failed = why, loading = false) }
            },
        )
    }

    /** The whole list a press plays from: what was drawn, or if the process has been away, briefly, a new load. */
    suspend fun forPress(context: Context, key: String): List<Track> {
        if (now(key).tracks.isEmpty()) {
            // Its own job, as giving up on the wait must not stop the load half way.
            val app = context.applicationContext
            val load = context.graph.scope.launch { refresh(app, key) }
            withTimeoutOrNull(PRESS_WAIT_MS) { load.join() }
        }
        return now(key).tracks
    }

    private fun change(key: String, how: (WidgetSongs) -> WidgetSongs) =
        lists.update { it + (key to how(it[key] ?: WidgetSongs())) }
}

/** Plays a list's song, by id, with the rest of the list after it. */
class PlaySongAction : ActionCallback {
    override suspend fun onAction(context: Context, glanceId: GlanceId, parameters: ActionParameters) {
        val key = parameters[LIST] ?: return
        val song = parameters[SONG] ?: return
        val graph = context.graph
        WidgetPlayback.ready(context)
        val tracks = WidgetLists.forPress(context, key).filter { it.playable }
        val origin = WidgetLists.now(key).title
        val start = startOf(tracks, song)
        // Gone from the list, or the list could not be had: the song that was tapped still plays.
        if (start != null) graph.playback.play(tracks, start, origin)
        else graph.playback.play(listOf(Track(id = song, title = parameters[TITLE].orEmpty())), 0, origin)
    }

    companion object {
        val LIST = ActionParameters.Key<String>("list")
        val SONG = ActionParameters.Key<String>("song")
        val TITLE = ActionParameters.Key<String>("title")
    }
}

/** Loads a list again, and draws every widget showing it. */
class RefreshListAction : ActionCallback {
    override suspend fun onAction(context: Context, glanceId: GlanceId, parameters: ActionParameters) {
        val key = parameters[PlaySongAction.LIST] ?: return
        // Off this callback's short leash: the load shows in the widget however long it takes.
        val graph = context.graph
        graph.scope.launch {
            WidgetLists.refresh(context.applicationContext, key)
            runCatching {
                RecentWidget().updateAll(context.applicationContext)
                PlaylistWidget().updateAll(context.applicationContext)
            }
        }
    }
}

/** The action of a refresh button for the list [key]. */
internal fun refreshList(key: String): Action =
    actionRunCallback<RefreshListAction>(actionParametersOf(PlaySongAction.LIST to key))

/** A widget's list: its heading, its buttons, what went wrong if something did, and its songs with their covers. */
@Composable
internal fun SongListBody(
    key: String,
    title: String,
    songs: WidgetSongs,
    colors: WidgetColors,
    empty: String,
    onTitle: Action,
    buttons: @Composable () -> Unit = {},
) {
    val rows = rowsOf(songs.tracks)
    Column(GlanceModifier.fillMaxSize().background(colors.background).cornerRadius(16.dp).padding(start = 12.dp, top = 6.dp, end = 4.dp, bottom = 8.dp)) {
        Row(GlanceModifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
            Text(
                title, maxLines = 1,
                modifier = GlanceModifier.defaultWeight().clickable(onTitle),
                style = TextStyle(color = colors.text, fontSize = 15.sp, fontWeight = FontWeight.Bold),
            )
            buttons()
            WidgetButton(R.drawable.ic_refresh_cw, "Refresh $title", if (songs.loading) colors.accent else colors.secondary, refreshList(key))
        }
        songs.failed?.let { Text(it, style = TextStyle(color = colors.secondary, fontSize = 13.sp)) }
        if (rows.isEmpty() && songs.failed == null) {
            Text(if (songs.loading) "Loading…" else empty, style = TextStyle(color = colors.secondary, fontSize = 13.sp))
        }
        LazyColumn {
            items(rows, itemId = { it.id.hashCode().toLong() }) { track -> SongRow(key, track, songs.art[track.id], colors) }
        }
    }
}

@Composable
private fun SongRow(key: String, track: Track, art: Bitmap?, colors: WidgetColors) {
    val play = actionRunCallback<PlaySongAction>(
        actionParametersOf(PlaySongAction.LIST to key, PlaySongAction.SONG to track.id, PlaySongAction.TITLE to track.title),
    )
    Row(GlanceModifier.fillMaxWidth().padding(vertical = 4.dp).clickable(play), verticalAlignment = Alignment.CenterVertically) {
        if (art != null) {
            Image(ImageProvider(art), null, GlanceModifier.size(40.dp).cornerRadius(4.dp))
        } else {
            Image(ImageProvider(R.drawable.ic_music), null, GlanceModifier.size(40.dp).padding(8.dp), colorFilter = ColorFilter.tint(colors.secondary))
        }
        Spacer(GlanceModifier.width(10.dp))
        Column(GlanceModifier.defaultWeight()) {
            Text(track.title, maxLines = 1, style = TextStyle(color = colors.text, fontSize = 14.sp))
            Text(track.artistNames, maxLines = 1, style = TextStyle(color = colors.secondary, fontSize = 12.sp))
        }
    }
}
