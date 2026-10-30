package dev.darksavci.spotified.widget

import android.appwidget.AppWidgetManager
import android.content.Context
import android.content.Intent
import android.net.Uri
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.glance.GlanceId
import androidx.glance.GlanceModifier
import androidx.glance.action.ActionParameters
import androidx.glance.action.actionParametersOf
import androidx.glance.action.clickable
import androidx.glance.appwidget.GlanceAppWidget
import androidx.glance.appwidget.GlanceAppWidgetManager
import androidx.glance.appwidget.GlanceAppWidgetReceiver
import androidx.glance.appwidget.action.ActionCallback
import androidx.glance.appwidget.action.actionRunCallback
import androidx.glance.appwidget.action.actionStartActivity
import androidx.glance.appwidget.cornerRadius
import androidx.glance.appwidget.provideContent
import androidx.glance.appwidget.state.getAppWidgetState
import androidx.glance.background
import androidx.glance.currentState
import androidx.glance.layout.Alignment
import androidx.glance.layout.Box
import androidx.glance.layout.fillMaxSize
import androidx.glance.layout.padding
import androidx.glance.text.Text
import androidx.glance.text.TextAlign
import androidx.glance.text.TextStyle
import dev.darksavci.spotified.R
import dev.darksavci.spotified.graph
import dev.darksavci.spotified.ui.Actions
import dev.darksavci.spotified.ui.MainActivity
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeoutOrNull

/** One playlist's songs on the home screen, chosen when the widget is placed: a tap plays from that song. */
class PlaylistWidget : GlanceAppWidget() {
    override suspend fun provideGlance(context: Context, id: GlanceId) {
        val graph = context.graph
        val app = context.applicationContext
        val chosen = getAppWidgetState<Preferences>(context, id)[PLAYLIST]
        if (chosen != null) {
            val load = graph.scope.launch { WidgetLists.freshen(app, WidgetLists.playlistKey(chosen)) }
            withTimeoutOrNull(FIRST_DRAW_WAIT_MS) { load.join() }
        }
        val first = graph.settings.current().theme
        val widgetId = GlanceAppWidgetManager(context).getAppWidgetId(id)
        val choose = Intent(app, PlaylistWidgetConfigure::class.java)
            .putExtra(AppWidgetManager.EXTRA_APPWIDGET_ID, widgetId)
            // Each widget's own request, or the system would hand back the first one's.
            .setData(Uri.parse("spotified-widget://choose/$widgetId"))
        provideContent {
            val theme by remember { themeOf(graph) }.collectAsState(first)
            val colors = WidgetColors(theme)
            val playlist = currentState<Preferences>()[PLAYLIST]
            if (playlist == null) {
                Box(
                    GlanceModifier.fillMaxSize().background(colors.background).cornerRadius(16.dp).padding(16.dp)
                        .clickable(actionStartActivity(choose)),
                    contentAlignment = Alignment.Center,
                ) {
                    Text("Choose a playlist", style = TextStyle(color = colors.text, fontSize = 14.sp, textAlign = TextAlign.Center))
                }
            } else {
                val key = WidgetLists.playlistKey(playlist)
                val songs by remember(key) { WidgetLists.of(key) }.collectAsState(WidgetLists.now(key))
                // The name it was chosen by, until its songs have been read.
                val title = songs.title.ifEmpty { currentState<Preferences>()[PLAYLIST_TITLE].orEmpty().ifEmpty { "Playlist" } }
                val open = Intent(Intent.ACTION_VIEW, Uri.parse(Actions.playlistLink(playlist)), app, MainActivity::class.java)
                SongListBody(
                    key, title, songs, colors,
                    empty = "This playlist has no songs yet.",
                    onTitle = actionStartActivity(open),
                ) {
                    WidgetButton(
                        R.drawable.ic_shuffle, "Shuffle $title", colors.secondary,
                        actionRunCallback<ShuffleListAction>(actionParametersOf(PlaySongAction.LIST to key)),
                    )
                }
            }
        }
    }

    companion object {
        val PLAYLIST = stringPreferencesKey("playlist")
        val PLAYLIST_TITLE = stringPreferencesKey("playlistTitle")
    }
}

class PlaylistWidgetReceiver : GlanceAppWidgetReceiver() {
    override val glanceAppWidget: GlanceAppWidget = PlaylistWidget()
}

/** Plays a whole list shuffled. */
class ShuffleListAction : ActionCallback {
    override suspend fun onAction(context: Context, glanceId: GlanceId, parameters: ActionParameters) {
        val key = parameters[PlaySongAction.LIST] ?: return
        WidgetPlayback.ready(context)
        val tracks = WidgetLists.forPress(context, key)
        context.graph.playback.shuffle(tracks, WidgetLists.now(key).title)
    }
}
