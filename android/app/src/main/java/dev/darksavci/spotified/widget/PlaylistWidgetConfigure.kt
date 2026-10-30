package dev.darksavci.spotified.widget

import android.app.Activity
import android.appwidget.AppWidgetManager
import android.content.ComponentName
import android.content.Intent
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.glance.appwidget.GlanceAppWidgetManager
import androidx.glance.appwidget.state.updateAppWidgetState
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.lifecycleScope
import dev.darksavci.spotified.api.LibraryItem
import dev.darksavci.spotified.api.LibraryKind
import dev.darksavci.spotified.data.Settings
import dev.darksavci.spotified.graph
import dev.darksavci.spotified.ui.components.Art
import dev.darksavci.spotified.ui.components.Problem
import dev.darksavci.spotified.ui.components.SkeletonList
import dev.darksavci.spotified.ui.components.tapRow
import dev.darksavci.spotified.ui.theme.LocalPalette
import dev.darksavci.spotified.ui.theme.SpotifiedTheme
import dev.darksavci.spotified.ui.theme.Type
import kotlinx.coroutines.launch

/** Asks which playlist a new Playlist widget shows, and again when it is reconfigured. */
class PlaylistWidgetConfigure : ComponentActivity() {
    private var widgetId = AppWidgetManager.INVALID_APPWIDGET_ID

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        // Backing out of this leaves no widget behind.
        setResult(Activity.RESULT_CANCELED)
        widgetId = intent?.getIntExtra(AppWidgetManager.EXTRA_APPWIDGET_ID, widgetId) ?: widgetId
        // Other apps can start this, so only a widget of this kind is changed.
        val ours = ComponentName(this, PlaylistWidgetReceiver::class.java)
        if (AppWidgetManager.getInstance(this).getAppWidgetInfo(widgetId)?.provider != ours) {
            finish()
            return
        }
        setContent {
            val settings by graph.settings.settings.collectAsStateWithLifecycle(Settings())
            SpotifiedTheme(settings.theme) { Chooser(::choose) }
        }
    }

    private fun choose(item: LibraryItem) {
        val context = applicationContext
        lifecycleScope.launch {
            val glance = runCatching { GlanceAppWidgetManager(context).getGlanceIdBy(widgetId) }.getOrNull()
            if (glance != null) {
                updateAppWidgetState(context, glance) {
                    it[PlaylistWidget.PLAYLIST] = item.id
                    it[PlaylistWidget.PLAYLIST_TITLE] = item.title
                }
                PlaylistWidget().update(context, glance)
                setResult(Activity.RESULT_OK, Intent().putExtra(AppWidgetManager.EXTRA_APPWIDGET_ID, widgetId))
            }
            finish()
        }
    }
}

@Composable
private fun Chooser(onChoose: (LibraryItem) -> Unit) {
    val p = LocalPalette.current
    val api = androidx.compose.ui.platform.LocalContext.current.graph.api
    var attempt by remember { mutableIntStateOf(0) }
    val found by produceState<Result<List<LibraryItem>>?>(null, attempt) {
        value = null
        value = runCatching { api.library().filter { it.kind == LibraryKind.Playlist } }
    }
    Column(Modifier.fillMaxSize().background(p.window).windowInsetsPadding(WindowInsets.safeDrawing)) {
        Text("Choose a playlist", style = Type.title, modifier = Modifier.padding(16.dp).semantics { heading() })
        val result = found
        when {
            result == null -> SkeletonList()
            result.isFailure -> Problem(result.exceptionOrNull()?.message ?: "Your library could not be read.", { attempt++ })
            result.getOrThrow().isEmpty() -> Problem("Your library has no playlists yet.", { attempt++ })
            else -> LazyColumn(contentPadding = PaddingValues(bottom = 16.dp)) {
                items(result.getOrThrow(), key = { it.id }) { item ->
                    Row(
                        Modifier.fillMaxWidth().tapRow(onClick = { onChoose(item) }, role = Role.Button)
                            .padding(horizontal = 16.dp, vertical = 8.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Art(item.artwork, 48.dp)
                        Spacer(Modifier.width(12.dp))
                        Column(Modifier.weight(1f)) {
                            Text(item.title, style = Type.body, maxLines = 1, overflow = TextOverflow.Ellipsis)
                            if (item.subtitle.isNotEmpty()) {
                                Text(item.subtitle, style = Type.small, color = p.secondary, maxLines = 1, overflow = TextOverflow.Ellipsis)
                            }
                        }
                    }
                }
            }
        }
    }
}
