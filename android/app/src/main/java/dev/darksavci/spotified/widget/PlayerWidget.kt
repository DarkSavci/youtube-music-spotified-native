package dev.darksavci.spotified.widget

import android.content.Context
import android.graphics.Bitmap
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.glance.ColorFilter
import androidx.glance.GlanceId
import androidx.glance.GlanceModifier
import androidx.glance.Image
import androidx.glance.ImageProvider
import androidx.glance.action.Action
import androidx.glance.action.ActionParameters
import androidx.glance.action.actionStartActivity
import androidx.glance.action.clickable
import androidx.glance.appwidget.GlanceAppWidget
import androidx.glance.appwidget.GlanceAppWidgetReceiver
import androidx.glance.appwidget.action.ActionCallback
import androidx.glance.appwidget.action.actionRunCallback
import androidx.glance.appwidget.cornerRadius
import androidx.glance.appwidget.provideContent
import androidx.glance.appwidget.updateAll
import androidx.glance.background
import androidx.glance.layout.Alignment
import androidx.glance.layout.Column
import androidx.glance.layout.Row
import androidx.glance.layout.Spacer
import androidx.glance.layout.fillMaxSize
import androidx.glance.layout.padding
import androidx.glance.layout.size
import androidx.glance.layout.width
import androidx.glance.text.FontWeight
import androidx.glance.text.Text
import androidx.glance.text.TextStyle
import androidx.glance.unit.ColorProvider
import coil3.imageLoader
import coil3.request.ImageRequest
import coil3.request.SuccessResult
import coil3.request.allowHardware
import coil3.toBitmap
import dev.darksavci.spotified.R
import dev.darksavci.spotified.api.url
import dev.darksavci.spotified.graph
import dev.darksavci.spotified.ui.MainActivity
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.FlowPreview
import kotlinx.coroutines.flow.debounce
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch

/** The player on the home screen: the song, its art, play, next and like. */
class PlayerWidget : GlanceAppWidget() {
    override suspend fun provideGlance(context: Context, id: GlanceId) {
        val graph = context.graph
        val projection = graph.playback.projection.value
        val track = projection.state.current
        val playing = projection.target.playing
        val liked = track != null && graph.liked.isLiked(track.id)
        val art = track?.artwork?.url(160)?.let { loadBitmap(context, it) }
        provideContent {
            Row(
                GlanceModifier.fillMaxSize().background(Color(0xFF181818)).cornerRadius(16.dp).padding(8.dp)
                    .clickable(actionStartActivity<MainActivity>()),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                if (art != null) {
                    Image(ImageProvider(art), null, GlanceModifier.size(48.dp).cornerRadius(6.dp))
                } else {
                    Image(ImageProvider(R.drawable.ic_music), null, GlanceModifier.size(48.dp), colorFilter = ColorFilter.tint(ColorProvider(Color.Gray)))
                }
                Spacer(GlanceModifier.width(10.dp))
                Column(GlanceModifier.defaultWeight()) {
                    Text(
                        track?.title ?: "Nothing playing", maxLines = 1,
                        style = TextStyle(color = ColorProvider(Color.White), fontSize = 14.sp, fontWeight = FontWeight.Bold),
                    )
                    Text(track?.artistNames.orEmpty(), maxLines = 1, style = TextStyle(color = ColorProvider(Color(0xFFAAAAAA)), fontSize = 12.sp))
                }
                if (track != null) {
                    Button(if (liked) R.drawable.ic_heart_filled else R.drawable.ic_heart, if (liked) Color(0xFFFF0033) else Color.White, actionRunCallback<LikeAction>())
                }
                Button(if (playing) R.drawable.ic_pause_filled else R.drawable.ic_play_filled, Color.White, actionRunCallback<ToggleAction>())
                Button(R.drawable.ic_skip_forward_filled, Color.White, actionRunCallback<NextAction>())
            }
        }
    }

    @Composable
    private fun Button(icon: Int, tint: Color, action: Action) {
        Image(
            ImageProvider(icon), null,
            GlanceModifier.size(40.dp).padding(8.dp).clickable(action),
            colorFilter = ColorFilter.tint(ColorProvider(tint)),
        )
    }

    private suspend fun loadBitmap(context: Context, url: String): Bitmap? {
        val request = ImageRequest.Builder(context).data(url).allowHardware(false).size(160).build()
        return (context.imageLoader.execute(request) as? SuccessResult)?.image?.toBitmap()
    }

    companion object {
        /** Keeps the widgets showing what plays, redrawn when the song or the play state changes. */
        @OptIn(FlowPreview::class)
        fun follow(context: Context, scope: CoroutineScope) {
            val graph = context.graph
            scope.launch {
                graph.playback.projection
                    .map { Triple(it.state.current?.id, it.target.playing, it.state.current?.let { t -> graph.liked.isLiked(t.id) }) }
                    .distinctUntilChanged()
                    .debounce(300)
                    .collect { runCatching { PlayerWidget().updateAll(context) } }
            }
        }
    }
}

class PlayerWidgetReceiver : GlanceAppWidgetReceiver() {
    override val glanceAppWidget: GlanceAppWidget = PlayerWidget()
}

class ToggleAction : ActionCallback {
    override suspend fun onAction(context: Context, glanceId: GlanceId, parameters: ActionParameters) {
        context.graph.playback.toggle()
    }
}

class NextAction : ActionCallback {
    override suspend fun onAction(context: Context, glanceId: GlanceId, parameters: ActionParameters) {
        context.graph.playback.next()
    }
}

class LikeAction : ActionCallback {
    override suspend fun onAction(context: Context, glanceId: GlanceId, parameters: ActionParameters) {
        context.graph.playback.projection.value.state.current?.let(context.graph.liked::toggle)
        PlayerWidget().updateAll(context)
    }
}
