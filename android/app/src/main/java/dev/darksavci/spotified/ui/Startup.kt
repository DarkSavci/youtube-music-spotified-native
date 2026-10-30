package dev.darksavci.spotified.ui

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import dev.darksavci.spotified.BuildConfig
import dev.darksavci.spotified.api.Command
import dev.darksavci.spotified.data.Changelog
import dev.darksavci.spotified.data.Crashes
import dev.darksavci.spotified.data.RemotePickup
import dev.darksavci.spotified.data.Updater
import dev.darksavci.spotified.graph
import dev.darksavci.spotified.ui.theme.LocalPalette
import dev.darksavci.spotified.ui.theme.Type
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.withContext

/** Decided once a process, not once a window: the activity can be made again. */
private var pickedUp = false

/** What happens once when the app opens: the quiet update check, the queue from another device, and What's new. */
@Composable
fun Startup(actions: Actions) {
    val context = LocalContext.current
    val graph = context.graph
    LaunchedEffect(Unit) { graph.settings.settings.map { it.reduceMotion }.collect { actions.reduceMotion = it } }
    LaunchedEffect(Unit) {
        val settings = graph.settings.current()
        if (settings.checkForUpdates) runCatching { Updater.lookQuietly(context, graph.http) }
        if (pickedUp) return@LaunchedEffect
        pickedUp = true
        if (settings.continueFromYouTubeMusic) pickUp(actions)
    }
    WhatsNewPrompt(actions)
}

/**
 * The queue on another device, put in place paused, and only if nothing started
 * playing here while it was read. Quiet: nobody asked at this moment, so a failed
 * or empty read says nothing. Already on the same song, it is left alone.
 */
private suspend fun pickUp(actions: Actions) {
    val graph = actions.graph
    graph.playback.connected.first { it }
    if (runCatching { graph.api.account() }.getOrNull() == null) return
    val found = runCatching { RemotePickup.pick(graph.api.remoteQueue()) }.getOrNull() ?: return
    val now = graph.playback.projection.value
    if (now.target.playing || now.state.current?.id == found.tracks[found.index].id) return
    actions.say("Picked up your queue from YouTube Music: ${found.tracks[found.index].title}")
    graph.playback.send(Command.Play(found.tracks, found.index, found.origin, paused = true))
}

/** After an update, what it brought. Once for each version, and not over the offer to report a crash. */
@Composable
private fun WhatsNewPrompt(actions: Actions) {
    val context = LocalContext.current
    val p = LocalPalette.current
    var releases by remember { mutableStateOf<List<Changelog.Release>>(emptyList()) }
    LaunchedEffect(Unit) {
        val found = withContext(Dispatchers.IO) {
            val info = context.packageManager.getPackageInfo(context.packageName, 0)
            val shown = Changelog.toShow(Changelog.read(context), Changelog.lastSeen(context), BuildConfig.VERSION_NAME, info.firstInstallTime == info.lastUpdateTime)
            Changelog.markSeen(context, BuildConfig.VERSION_NAME)
            shown
        }
        if (Crashes.pending(context) == null) releases = found
    }
    if (releases.isEmpty()) return
    AlertDialog(
        onDismissRequest = { releases = emptyList() },
        containerColor = p.overlay,
        titleContentColor = p.text,
        textContentColor = p.secondary,
        title = { Text("What's new in ${releases.first().version}", style = Type.heading) },
        text = {
            Column(Modifier.heightIn(max = 360.dp).verticalScroll(rememberScrollState())) {
                releases.first().groups.forEach { group ->
                    Text(group.title, style = Type.bodyStrong, color = p.text, modifier = Modifier.padding(top = 8.dp, bottom = 4.dp))
                    group.items.forEach { Text("• $it", style = Type.small, modifier = Modifier.padding(bottom = 4.dp)) }
                }
            }
        },
        confirmButton = { TextButton({ releases = emptyList() }) { Text("Got it", style = Type.bodyStrong, color = p.accent) } },
        dismissButton = { TextButton({ releases = emptyList(); actions.go(Routes.WHATS_NEW) }) { Text("See all", style = Type.bodyStrong, color = p.text) } },
    )
}
