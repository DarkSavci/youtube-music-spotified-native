package dev.darksavci.spotified.ui.player

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.Canvas
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.lifecycle.compose.LifecycleStartEffect
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import dev.darksavci.spotified.playback.Engine

/**
 * The bars on Now Playing. The engine works them out only while this is on a
 * screen someone can see. They are read while drawing, so the bars change without
 * the page around them being composed again; pausing lets them fall.
 */
@Composable
internal fun SpectrumBars(engine: Engine?, playing: Boolean, color: Color, modifier: Modifier = Modifier) {
    if (engine == null) return
    LifecycleStartEffect(engine) {
        engine.watchSpectrum(true)
        onStopOrDispose { engine.watchSpectrum(false) }
    }
    val spectrum by engine.spectrum.collectAsStateWithLifecycle()
    val level by animateFloatAsState(if (playing) 1f else 0f, label = "bars")
    Canvas(modifier) {
        val bars = spectrum
        if (bars.isEmpty()) return@Canvas
        val slot = size.width / bars.size
        val barWidth = slot * 0.6f
        val corner = CornerRadius(barWidth / 2)
        for (i in bars.indices) {
            val height = (bars[i] * level * size.height).coerceAtLeast(barWidth)
            drawRoundRect(color, Offset(i * slot + (slot - barWidth) / 2, size.height - height), Size(barWidth, height), corner)
        }
    }
}
