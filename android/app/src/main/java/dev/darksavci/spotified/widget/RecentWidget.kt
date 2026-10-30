package dev.darksavci.spotified.widget

import android.content.Context
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.glance.GlanceId
import androidx.glance.action.actionStartActivity
import androidx.glance.appwidget.GlanceAppWidget
import androidx.glance.appwidget.GlanceAppWidgetReceiver
import androidx.glance.appwidget.provideContent
import dev.darksavci.spotified.graph
import dev.darksavci.spotified.ui.MainActivity
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeoutOrNull

/** How long a widget's first drawing waits for its list; after that the list arrives by itself. */
internal const val FIRST_DRAW_WAIT_MS = 8000L

/** What was played lately, on the home screen, with covers; a tap plays from that song. */
class RecentWidget : GlanceAppWidget() {
    override suspend fun provideGlance(context: Context, id: GlanceId) {
        val graph = context.graph
        val app = context.applicationContext
        val load = graph.scope.launch { WidgetLists.freshen(app, WidgetLists.RECENT) }
        withTimeoutOrNull(FIRST_DRAW_WAIT_MS) { load.join() }
        val first = graph.settings.current().theme
        provideContent {
            val songs by remember { WidgetLists.of(WidgetLists.RECENT) }.collectAsState(WidgetLists.now(WidgetLists.RECENT))
            val theme by remember { themeOf(graph) }.collectAsState(first)
            SongListBody(
                WidgetLists.RECENT, "Recently played", songs, WidgetColors(theme),
                empty = "Songs you play show up here.",
                onTitle = actionStartActivity<MainActivity>(),
            )
        }
    }
}

class RecentWidgetReceiver : GlanceAppWidgetReceiver() {
    override val glanceAppWidget: GlanceAppWidget = RecentWidget()
}
