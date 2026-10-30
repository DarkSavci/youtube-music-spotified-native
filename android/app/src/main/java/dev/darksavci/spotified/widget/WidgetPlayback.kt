package dev.darksavci.spotified.widget

import android.os.Build
import android.app.ApplicationStartInfo
import android.app.ActivityManager
import android.content.ComponentName
import android.content.Context
import androidx.media3.session.MediaController
import androidx.media3.session.SessionToken
import com.google.common.util.concurrent.ListenableFuture
import dev.darksavci.spotified.graph
import dev.darksavci.spotified.playback.PlaybackService
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.guava.await
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeoutOrNull

/**
 * Gets playback ready for a press on a widget, with the app perhaps closed.
 *
 * The core keeps the session, so a press changes what it wants, but the sound
 * comes from the engine, and the engine lives in [PlaybackService], which until
 * now only an open screen started: the button changed with no sound. Binding is
 * allowed from the background, so a controller connects as the app's screens do.
 * The service then puts itself in the foreground when the core says to play,
 * which Android allows for a moment after a press on a widget. That moment is
 * why this waits for as little as it can.
 */
internal object WidgetPlayback {
    /** How long a press waits for the service and the session before it is sent anyway. */
    private const val READY_MS = 8000L

    /** How long the service is held after a press, for the core's answer to arrive and start it. */
    private const val HOLD_MS = 30_000L

    private var controller: ListenableFuture<MediaController>? = null
    private var holding: Job? = null

    private var wakeLookedAt = false

    /**
     * Since Android 15 a stopped app's widgets are masked by the system with its icon,
     * and a tap on the mask only sends the provider an update: none of the widget's own
     * buttons exist to be pressed. So a process that this update started, straight after
     * a force stop, was started by a press, and is the one chance to answer it. Anything
     * else that unstops the app starts it by another way, and an update later in a
     * process's life is only an update.
     */
    fun playIfWokenByTap(context: Context) {
        if (wakeLookedAt) return
        wakeLookedAt = true
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.VANILLA_ICE_CREAM) return
        val start = context.getSystemService(ActivityManager::class.java).getHistoricalProcessStartReasons(1).firstOrNull() ?: return
        val mine = start.intent?.component?.className == PlayerWidgetReceiver::class.java.name
        if (!start.wasForceStopped() || start.reason != ApplicationStartInfo.START_REASON_BROADCAST || !mine) return
        val graph = context.graph
        graph.scope.launch {
            ready(context)
            val now = graph.playback.projection.value
            if (now.state.current != null && !now.target.playing) graph.playback.toggle()
        }
    }

    /** Starts the service if it is not running, and waits for it and for the session to answer. */
    suspend fun ready(context: Context) {
        val graph = context.graph
        withTimeoutOrNull(READY_MS) {
            withContext(Dispatchers.Main) { connect(context.applicationContext) }
            graph.playback.connected.first { it }
        }
    }

    private suspend fun connect(context: Context) {
        val future = controller?.takeUnless { it.isCancelled } ?: MediaController.Builder(
            context, SessionToken(context, ComponentName(context, PlaybackService::class.java)),
        ).buildAsync().also { controller = it }
        hold(context)
        runCatching { future.await() }.onFailure { controller = null }
    }

    /**
     * Keeps the service bound past the press, and while it plays. Letting go is
     * safe then: a service that went to the foreground stays, and one that never
     * had anything to play ends, leaving nothing running.
     */
    private fun hold(context: Context) {
        val graph = context.graph
        holding?.cancel()
        holding = graph.scope.launch(Dispatchers.Main) {
            delay(HOLD_MS)
            graph.playback.projection.first { !it.target.playing }
            controller?.let(MediaController::releaseFuture)
            controller = null
        }
    }
}
