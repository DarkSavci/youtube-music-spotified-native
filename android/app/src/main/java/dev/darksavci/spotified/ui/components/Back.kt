package dev.darksavci.spotified.ui.components

import androidx.activity.compose.PredictiveBackHandler
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.tween
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.launch

/**
 * A layer that follows the back gesture: it catches back while [enabled], the
 * returned function reads how far the gesture has gone (0 to 1, for
 * [predictiveBackLayer]), and [onBack] runs when the gesture commits. [onBack]
 * should close the layer, which turns [enabled] off and settles the value at 0;
 * a gesture given up settles it too.
 */
@Composable
fun predictiveBack(enabled: Boolean, onBack: () -> Unit): () -> Float {
    val progress = remember { Animatable(0f) }
    val scope = rememberCoroutineScope()
    val latest by rememberUpdatedState(onBack)
    PredictiveBackHandler(enabled) { events ->
        try {
            events.collect { progress.snapTo(it.progress) }
            latest()
        } catch (e: CancellationException) {
            scope.launch { progress.animateTo(0f) }
            throw e
        }
    }
    LaunchedEffect(enabled) { if (!enabled) progress.animateTo(0f, tween(300)) }
    return remember { { progress.value } }
}

/** Shrinks a full-screen layer and rounds its corners as a back gesture goes on. */
fun Modifier.predictiveBackLayer(progress: () -> Float): Modifier = graphicsLayer {
    val gone = progress()
    val scale = 1f - 0.1f * gone
    scaleX = scale
    scaleY = scale
    if (gone > 0f) {
        shape = RoundedCornerShape(28.dp * gone)
        clip = true
    }
}
