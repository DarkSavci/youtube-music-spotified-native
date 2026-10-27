package dev.darksavci.spotified.widget

import android.content.Context
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.glance.GlanceId
import androidx.glance.GlanceModifier
import androidx.glance.action.ActionParameters
import androidx.glance.action.actionParametersOf
import androidx.glance.action.clickable
import androidx.glance.appwidget.GlanceAppWidget
import androidx.glance.appwidget.GlanceAppWidgetReceiver
import androidx.glance.appwidget.action.ActionCallback
import androidx.glance.appwidget.action.actionRunCallback
import androidx.glance.appwidget.cornerRadius
import androidx.glance.appwidget.lazy.LazyColumn
import androidx.glance.appwidget.lazy.itemsIndexed
import androidx.glance.appwidget.provideContent
import androidx.glance.background
import androidx.glance.layout.Column
import androidx.glance.layout.fillMaxSize
import androidx.glance.layout.fillMaxWidth
import androidx.glance.layout.padding
import androidx.glance.text.FontWeight
import androidx.glance.text.Text
import androidx.glance.text.TextStyle
import androidx.glance.unit.ColorProvider
import dev.darksavci.spotified.api.Track
import dev.darksavci.spotified.graph

/** What was played lately, on the home screen; a tap plays from that song. */
class RecentWidget : GlanceAppWidget() {
    override suspend fun provideGlance(context: Context, id: GlanceId) {
        val graph = context.graph
        val recent = runCatching {
            graph.core.running()
            graph.api.history().distinctBy { it.id }.take(12)
        }.getOrDefault(emptyList())
        provideContent { Recent(recent) }
    }

    @Composable
    private fun Recent(tracks: List<Track>) {
        Column(GlanceModifier.fillMaxSize().background(Color(0xFF181818)).cornerRadius(16.dp).padding(12.dp)) {
            Text("Recently played", style = TextStyle(color = ColorProvider(Color.White), fontSize = 15.sp, fontWeight = FontWeight.Bold))
            if (tracks.isEmpty()) {
                Text("Nothing yet", style = TextStyle(color = ColorProvider(Color(0xFFAAAAAA)), fontSize = 13.sp))
            }
            LazyColumn {
                itemsIndexed(tracks) { i, t ->
                    Column(
                        GlanceModifier.fillMaxWidth().padding(vertical = 6.dp)
                            .clickable(actionRunCallback<PlayRecentAction>(actionParametersOf(INDEX to i))),
                    ) {
                        Text(t.title, maxLines = 1, style = TextStyle(color = ColorProvider(Color.White), fontSize = 14.sp))
                        Text(t.artistNames, maxLines = 1, style = TextStyle(color = ColorProvider(Color(0xFFAAAAAA)), fontSize = 12.sp))
                    }
                }
            }
        }
    }

    companion object {
        val INDEX = ActionParameters.Key<Int>("index")
    }
}

class RecentWidgetReceiver : GlanceAppWidgetReceiver() {
    override val glanceAppWidget: GlanceAppWidget = RecentWidget()
}

class PlayRecentAction : ActionCallback {
    override suspend fun onAction(context: Context, glanceId: GlanceId, parameters: ActionParameters) {
        val graph = context.graph
        graph.core.running()
        val recent = graph.api.history().distinctBy { it.id }.take(12)
        graph.playback.play(recent, parameters[RecentWidget.INDEX] ?: 0, "Recently played")
    }
}
