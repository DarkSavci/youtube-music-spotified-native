package dev.darksavci.spotified.ui.components

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.material3.pulltorefresh.PullToRefreshDefaults
import androidx.compose.material3.pulltorefresh.rememberPullToRefreshState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import dev.darksavci.spotified.ui.theme.LocalPalette
import kotlinx.coroutines.flow.combine

/**
 * Pull down to load [loaders] again, keeping what shows. Wrap a page's
 * scrolling list in it, the pull is read from that list. A failed pull keeps
 * the content and says so in a message.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun Refreshable(vararg loaders: Loader<*>, modifier: Modifier = Modifier, content: @Composable BoxScope.() -> Unit) {
    val p = LocalPalette.current
    val haptics = rememberHaptics()
    val state = rememberPullToRefreshState()
    val all = loaders.toList()
    val refreshing by remember(all) { combine(all.map { it.refreshing }) { flags -> flags.any { it } } }
        .collectAsStateWithLifecycle(false)
    PullToRefreshBox(
        isRefreshing = refreshing,
        onRefresh = {
            haptics.dragStart()
            all.forEach { it.reload(quietly = true) }
        },
        modifier = modifier.fillMaxSize(),
        state = state,
        indicator = {
            PullToRefreshDefaults.Indicator(
                state, refreshing, Modifier.align(Alignment.TopCenter).windowInsetsPadding(WindowInsets.statusBars),
                containerColor = p.overlay, color = p.accent,
            )
        },
        content = content,
    )
}

/** What a page shows of a load: [state], and whether it is old content kept while the next arrives. */
class Kept<T>(val state: Load<T>, val changing: Boolean)

/**
 * A load whose page keeps showing its last content when the [loader] changes
 * for another key, such as a mood chip or a period. Not yet having any shows as
 * Pending; a failure shows, since the old content is not what was asked for.
 */
@Composable
fun <T> keptState(loader: Loader<T>): Kept<T> {
    // Collected to follow it; read directly because a collection keeps its old
    // value for a frame when the loader changes, which would be the old key's.
    loader.state.collectAsStateWithLifecycle().value
    return keptOf(loader.state.value)
}

@Composable
private fun <T> keptOf(state: Load<T>): Kept<T> {
    val memory = remember { LastReady<T>() }
    if (state is Load.Ready) memory.value = state
    val last = memory.value
    return if (state is Load.Pending && last != null) Kept(last, changing = true) else Kept(state, changing = false)
}

private class LastReady<T> { var value: Load.Ready<T>? = null }

/** Dims old content while new content loads. */
fun Modifier.dimmedWhile(changing: Boolean): Modifier = if (changing) graphicsLayer { alpha = 0.55f } else this

/** A thin line that moves while kept content is being replaced. It always takes its height, so nothing jumps. */
@Composable
fun ChangingBar(changing: Boolean, modifier: Modifier = Modifier) {
    val p = LocalPalette.current
    Box(modifier.fillMaxWidth().height(2.dp)) {
        if (changing) LinearProgressIndicator(Modifier.fillMaxSize(), color = p.accent, trackColor = p.surface)
    }
}

/**
 * Shows a load: [skeleton] (or a spinner) while nothing is there yet, the
 * problem with a retry, or [content]. With [keep], content stays, dimmed under a
 * moving line, while another key loads.
 */
@Composable
fun <T> Loaded(
    loader: Loader<T>,
    skeleton: (@Composable () -> Unit)? = null,
    keep: Boolean = false,
    content: @Composable (T) -> Unit,
) {
    loader.state.collectAsStateWithLifecycle().value
    val state = loader.state.value
    val kept = keptOf(state)
    when (val s = if (keep) kept.state else state) {
        Load.Pending -> if (skeleton != null) skeleton() else Loading()
        is Load.Failed -> Problem(s.message, { loader.reload() })
        // Always in the same box when kept, so that the content keeps its scroll through a change.
        is Load.Ready -> if (keep) {
            Box(Modifier.fillMaxSize()) {
                Box(Modifier.dimmedWhile(kept.changing)) { content(s.value) }
                if (kept.changing) ChangingBar(true, Modifier.align(Alignment.TopCenter))
            }
        } else {
            content(s.value)
        }
    }
}
