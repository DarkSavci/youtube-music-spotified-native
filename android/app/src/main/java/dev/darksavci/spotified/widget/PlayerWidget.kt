package dev.darksavci.spotified.widget

import android.appwidget.AppWidgetManager
import android.content.Context
import android.graphics.Bitmap
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.DpSize
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.glance.ColorFilter
import androidx.glance.GlanceId
import androidx.glance.GlanceModifier
import androidx.glance.Image
import androidx.glance.ImageProvider
import androidx.glance.LocalSize
import androidx.glance.action.Action
import androidx.glance.action.ActionParameters
import androidx.glance.action.actionStartActivity
import androidx.glance.action.clickable
import androidx.glance.appwidget.GlanceAppWidget
import androidx.glance.appwidget.GlanceAppWidgetReceiver
import androidx.glance.appwidget.SizeMode
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
import androidx.glance.layout.fillMaxWidth
import androidx.glance.layout.height
import androidx.glance.layout.padding
import androidx.glance.layout.size
import androidx.glance.layout.width
import androidx.glance.text.FontWeight
import androidx.glance.text.Text
import androidx.glance.text.TextStyle
import dev.darksavci.spotified.Graph
import dev.darksavci.spotified.R
import dev.darksavci.spotified.api.Track
import dev.darksavci.spotified.data.Theme
import dev.darksavci.spotified.graph
import dev.darksavci.spotified.ui.MainActivity
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.FlowPreview
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.debounce
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.drop
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeoutOrNull

/** How long a like waits for the session with the app closed. */
private const val LIKE_WAIT_MS = 5000L

/** The size of the player's cover, in pixels: enough for its largest layout. */
private const val PLAYER_ART_PX = 240

/** The player's layouts, each with the smallest size it fits, as the launcher chooses among them. */
internal enum class PlayerLayout(val size: DpSize) {
    /** A cover and play. */
    Compact(DpSize(110.dp, 60.dp)),

    /** The song, play and next, in a line. */
    Line(DpSize(200.dp, 60.dp)),

    /** The same with room for like and previous. */
    WideLine(DpSize(300.dp, 60.dp)),

    /** The song above every button. */
    Tall(DpSize(200.dp, 110.dp));

    companion object {
        fun of(size: DpSize): PlayerLayout = when {
            size.width < Line.size.width -> Compact
            size.height >= Tall.size.height -> Tall
            size.width >= WideLine.size.width -> WideLine
            else -> Line
        }
    }
}

/** What the player widget shows of playback. */
private data class PlayerLook(val track: Track?, val playing: Boolean, val liked: Boolean)

private fun lookOf(graph: Graph): Flow<PlayerLook> =
    combine(graph.playback.projection, graph.liked.ids) { p, liked ->
        val track = p.state.current
        PlayerLook(track, p.target.playing, track != null && track.id in liked)
    }.distinctUntilChanged()

/** The theme chosen in Settings, as it changes. */
internal fun themeOf(graph: Graph): Flow<Theme> = graph.settings.settings.map { it.theme }.distinctUntilChanged()

/** The player on the home screen: the song, its art and the controls that fit. */
class PlayerWidget : GlanceAppWidget() {
    override val sizeMode: SizeMode = SizeMode.Responsive(PlayerLayout.entries.map { it.size }.toSet())

    /*
     * What shows is read inside the content, not before it: while a widget's
     * session lasts, an update only draws the content again, so anything read
     * out here would stay as it was.
     */
    override suspend fun provideGlance(context: Context, id: GlanceId) {
        val graph = context.graph
        val first = PlayerLook(
            graph.playback.projection.value.state.current,
            graph.playback.projection.value.target.playing,
            graph.playback.projection.value.state.current?.let { graph.liked.isLiked(it.id) } ?: false,
        )
        val firstTheme = graph.settings.current().theme
        val firstArt = first.track?.let { widgetBitmap(context, it.artwork, PLAYER_ART_PX) }
        provideContent {
            val look by remember { lookOf(graph) }.collectAsState(first)
            val theme by remember { themeOf(graph) }.collectAsState(firstTheme)
            val art by produceState(firstArt, look.track?.id) {
                value = look.track?.let { widgetBitmap(context, it.artwork, PLAYER_ART_PX) }
            }
            Player(look, art, WidgetColors(theme), PlayerLayout.of(LocalSize.current))
        }
    }

    @Composable
    private fun Player(look: PlayerLook, art: Bitmap?, colors: WidgetColors, layout: PlayerLayout) {
        val background = GlanceModifier.fillMaxSize().background(colors.background).cornerRadius(16.dp).padding(8.dp)
            .clickable(actionStartActivity<MainActivity>())
        when (layout) {
            PlayerLayout.Tall -> Column(background) {
                Row(GlanceModifier.fillMaxWidth().defaultWeight(), verticalAlignment = Alignment.CenterVertically) {
                    Cover(look, art, colors, 64.dp)
                    Spacer(GlanceModifier.width(10.dp))
                    Words(look, colors, GlanceModifier.defaultWeight())
                }
                Spacer(GlanceModifier.height(4.dp))
                Row(GlanceModifier.fillMaxWidth(), horizontalAlignment = Alignment.CenterHorizontally, verticalAlignment = Alignment.CenterVertically) {
                    Like(look, colors)
                    Previous(colors)
                    Play(look, colors)
                    Next(colors)
                }
            }
            else -> Row(background, verticalAlignment = Alignment.CenterVertically) {
                Cover(look, art, colors, if (layout == PlayerLayout.Compact) 40.dp else 48.dp)
                Spacer(GlanceModifier.width(10.dp))
                if (layout == PlayerLayout.Compact) {
                    Spacer(GlanceModifier.defaultWeight())
                } else {
                    Words(look, colors, GlanceModifier.defaultWeight())
                }
                if (layout == PlayerLayout.WideLine) {
                    Like(look, colors)
                    Previous(colors)
                }
                Play(look, colors)
                if (layout != PlayerLayout.Compact) Next(colors)
            }
        }
    }

    @Composable
    private fun Cover(look: PlayerLook, art: Bitmap?, colors: WidgetColors, size: Dp) {
        val said = look.track?.let { "${it.title}, ${it.artistNames}" } ?: "Nothing playing"
        if (art != null) {
            Image(ImageProvider(art), said, GlanceModifier.size(size).cornerRadius(6.dp))
        } else {
            Image(ImageProvider(R.drawable.ic_music), said, GlanceModifier.size(size).padding(8.dp), colorFilter = ColorFilter.tint(colors.secondary))
        }
    }

    @Composable
    private fun Words(look: PlayerLook, colors: WidgetColors, modifier: GlanceModifier) {
        Column(modifier) {
            Text(
                look.track?.title ?: "Nothing playing", maxLines = 1,
                style = TextStyle(color = colors.text, fontSize = 14.sp, fontWeight = FontWeight.Bold),
            )
            Text(look.track?.artistNames.orEmpty(), maxLines = 1, style = TextStyle(color = colors.secondary, fontSize = 12.sp))
        }
    }

    /** With nothing to play, the app is where to choose something. */
    @Composable
    private fun Play(look: PlayerLook, colors: WidgetColors) {
        val action: Action = if (look.track != null) actionRunCallback<ToggleAction>() else actionStartActivity<MainActivity>()
        WidgetButton(
            if (look.playing) R.drawable.ic_pause_filled else R.drawable.ic_play_filled,
            if (look.playing) "Pause" else "Play", colors.text, action,
        )
    }

    @Composable
    private fun Next(colors: WidgetColors) = WidgetButton(R.drawable.ic_skip_forward_filled, "Next song", colors.text, actionRunCallback<NextAction>())

    @Composable
    private fun Previous(colors: WidgetColors) = WidgetButton(R.drawable.ic_skip_back_filled, "Previous song", colors.text, actionRunCallback<PreviousAction>())

    @Composable
    private fun Like(look: PlayerLook, colors: WidgetColors) {
        if (look.track == null) return
        WidgetButton(
            if (look.liked) R.drawable.ic_heart_filled else R.drawable.ic_heart,
            if (look.liked) "Remove from Liked" else "Like",
            if (look.liked) colors.accent else colors.text,
            actionRunCallback<LikeAction>(),
        )
    }

    companion object {
        /** Keeps the widgets showing what plays and the theme chosen, starting a drawing when none is under way. */
        @OptIn(FlowPreview::class)
        fun follow(context: Context, scope: CoroutineScope) {
            val graph = context.graph
            scope.launch {
                lookOf(graph).debounce(300).collect { runCatching { PlayerWidget().updateAll(context) } }
            }
            scope.launch {
                themeOf(graph).drop(1).collect {
                    runCatching {
                        PlayerWidget().updateAll(context)
                        RecentWidget().updateAll(context)
                        PlaylistWidget().updateAll(context)
                    }
                }
            }
        }
    }
}

class PlayerWidgetReceiver : GlanceAppWidgetReceiver() {
    override val glanceAppWidget: GlanceAppWidget = PlayerWidget()

    override fun onUpdate(context: Context, appWidgetManager: AppWidgetManager, appWidgetIds: IntArray) {
        super.onUpdate(context, appWidgetManager, appWidgetIds)
        WidgetPlayback.playIfWokenByTap(context)
    }
}

class ToggleAction : ActionCallback {
    override suspend fun onAction(context: Context, glanceId: GlanceId, parameters: ActionParameters) {
        WidgetPlayback.ready(context)
        context.graph.playback.toggle()
    }
}

class NextAction : ActionCallback {
    override suspend fun onAction(context: Context, glanceId: GlanceId, parameters: ActionParameters) {
        WidgetPlayback.ready(context)
        context.graph.playback.next()
    }
}

class PreviousAction : ActionCallback {
    override suspend fun onAction(context: Context, glanceId: GlanceId, parameters: ActionParameters) {
        WidgetPlayback.ready(context)
        context.graph.playback.previous()
    }
}

class LikeAction : ActionCallback {
    override suspend fun onAction(context: Context, glanceId: GlanceId, parameters: ActionParameters) {
        // With the app closed, what plays is not known until the session answers.
        withTimeoutOrNull(LIKE_WAIT_MS) { context.graph.playback.connected.first { it } }
        context.graph.playback.projection.value.state.current?.let(context.graph.liked::toggle)
        PlayerWidget().updateAll(context)
    }
}
