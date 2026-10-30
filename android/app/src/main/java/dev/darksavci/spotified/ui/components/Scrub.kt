package dev.darksavci.spotified.ui.components

import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.pointer.changedToUpIgnoreConsumed
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.input.pointer.positionChange
import androidx.compose.ui.semantics.ProgressBarRangeInfo
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.progressBarRangeInfo
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.setProgress
import kotlin.math.abs

/** Whether a touch that has moved is a drag along a slider, or a page scrolling under it. */
internal class ScrubIntent(private val slop: Float) {
    enum class Verdict { Undecided, Scrub, Scroll }

    private var x = 0f
    private var y = 0f

    fun moved(dx: Float, dy: Float): Verdict {
        x += dx
        y += dy
        return when {
            abs(x) > slop && abs(x) >= abs(y) -> Verdict.Scrub
            abs(y) > slop -> Verdict.Scroll
            else -> Verdict.Undecided
        }
    }
}

/**
 * The touch of a slider or seek bar, for a horizontal line as wide as the
 * element. Nothing changes on touching it, so a touch that becomes a page
 * scroll leaves the value alone. A tap calls [onMove] then [onDone]. A drag
 * calls [onHeld] with true, [onMove] as it goes, then [onHeld] with false and
 * [onDone] once, even if it is cut short. [onMove] is given 0 to 1 along the width.
 */
@Composable
fun Modifier.scrub(
    enabled: Boolean = true,
    onHeld: (Boolean) -> Unit = {},
    onMove: (Float) -> Unit,
    onDone: () -> Unit,
): Modifier {
    val held by rememberUpdatedState(onHeld)
    val move by rememberUpdatedState(onMove)
    val done by rememberUpdatedState(onDone)
    return pointerInput(enabled) {
        if (!enabled) return@pointerInput
        awaitEachGesture {
            val down = awaitFirstDown()
            val intent = ScrubIntent(viewConfiguration.touchSlop)
            var scrubbing = false
            fun along(x: Float) = if (size.width > 0) (x / size.width).coerceIn(0f, 1f) else 0f
            try {
                while (true) {
                    val change = awaitPointerEvent().changes.firstOrNull { it.id == down.id } ?: return@awaitEachGesture
                    if (change.changedToUpIgnoreConsumed()) {
                        if (!scrubbing && !change.isConsumed) {
                            change.consume()
                            move(along(change.position.x))
                            done()
                        }
                        return@awaitEachGesture
                    }
                    // Something else, a page that has started scrolling, took this touch.
                    if (!scrubbing && change.isConsumed) return@awaitEachGesture
                    val moved = change.positionChange()
                    if (!scrubbing) {
                        when (intent.moved(moved.x, moved.y)) {
                            ScrubIntent.Verdict.Scroll -> return@awaitEachGesture
                            ScrubIntent.Verdict.Undecided -> continue
                            ScrubIntent.Verdict.Scrub -> {
                                scrubbing = true
                                held(true)
                            }
                        }
                    }
                    if (moved.x != 0f) move(along(change.position.x))
                    change.consume()
                }
            } finally {
                if (scrubbing) {
                    held(false)
                    done()
                }
            }
        }
    }
}

/** What TalkBack reads and sets of a [scrub]bed line: [fraction] is 0 to 1. */
fun Modifier.scrubSemantics(
    fraction: Float,
    description: String,
    onMove: (Float) -> Unit,
    onDone: () -> Unit,
    enabled: Boolean = true,
): Modifier = semantics(mergeDescendants = true) {
    contentDescription = description
    progressBarRangeInfo = ProgressBarRangeInfo(fraction, 0f..1f)
    if (enabled) {
        setProgress { target ->
            onMove(target.coerceIn(0f, 1f))
            onDone()
            true
        }
    }
}
