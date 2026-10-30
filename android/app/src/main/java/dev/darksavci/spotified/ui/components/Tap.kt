package dev.darksavci.spotified.ui.components

import androidx.compose.foundation.focusable
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.indication
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.PressInteraction
import androidx.compose.material3.ripple
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.Modifier
import androidx.compose.ui.composed
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.input.pointer.AwaitPointerEventScope
import androidx.compose.ui.input.pointer.PointerEventPass
import androidx.compose.ui.input.pointer.PointerId
import androidx.compose.ui.input.pointer.changedToUp
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.disabled
import androidx.compose.ui.semantics.onClick
import androidx.compose.ui.semantics.onLongClick
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

/** How long a finger must rest before a press shows, so that the start of a scroll does not flash the row. */
private const val PRESS_SHOWS_AFTER_MS = 80L

/**
 * A tap on a row, with its ripple and press state. Unlike `clickable`, a finger
 * that moves past the touch slop in any direction has stopped being a tap, even
 * if it lifts inside the row, so a sideways swipe over a row does nothing to it.
 * [onLongClick] buzzes by itself, as `combinedClickable` does.
 */
fun Modifier.tapRow(
    onClick: () -> Unit,
    onLongClick: (() -> Unit)? = null,
    enabled: Boolean = true,
    role: Role? = null,
    onClickLabel: String? = null,
    onLongClickLabel: String? = null,
): Modifier = composed {
    val interactions = remember { MutableInteractionSource() }
    val scope = rememberCoroutineScope()
    val haptics = LocalHapticFeedback.current
    val tap by rememberUpdatedState(onClick)
    val hold by rememberUpdatedState(onLongClick)
    val holds = onLongClick != null
    this
        .semantics(mergeDescendants = true) {
            if (role != null) this.role = role
            if (!enabled) disabled()
            onClick(onClickLabel) { if (enabled) tap(); enabled }
            if (holds) onLongClick(onLongClickLabel) { if (enabled) hold?.invoke(); enabled }
        }
        .onKeyEvent { event ->
            val press = enabled && event.type == KeyEventType.KeyUp &&
                (event.key == Key.Enter || event.key == Key.DirectionCenter || event.key == Key.NumPadEnter)
            if (press) tap()
            press
        }
        .indication(interactions, ripple())
        .focusable(enabled, interactions)
        .pointerInput(enabled, holds) {
            if (!enabled) return@pointerInput
            val slop = viewConfiguration.touchSlop
            val holdMs = viewConfiguration.longPressTimeoutMillis
            awaitEachGesture {
                val down = awaitFirstDown()
                val press = PressInteraction.Press(down.position)
                var shown = false
                val showing: Job = scope.launch {
                    delay(PRESS_SHOWS_AFTER_MS)
                    shown = interactions.tryEmit(press)
                }
                // Nothing is consumed before the end, so that a list or a sheet around the row can take the drag.
                val first = if (holds) {
                    withTimeoutOrNull(holdMs) { pressEnd(down.id, down.position, slop) } ?: PressEnd.Held
                } else {
                    pressEnd(down.id, down.position, slop)
                }
                showing.cancel()
                when (first) {
                    PressEnd.Moved -> if (shown) interactions.tryEmit(PressInteraction.Cancel(press))
                    PressEnd.Lifted -> {
                        // A tap quicker than the delay still shows its ripple.
                        if (!shown) interactions.tryEmit(press)
                        interactions.tryEmit(PressInteraction.Release(press))
                        tap()
                    }
                    PressEnd.Held -> {
                        if (!shown) interactions.tryEmit(press)
                        haptics.performHapticFeedback(HapticFeedbackType.LongPress)
                        hold?.invoke()
                        // The rest of this touch belongs to the hold.
                        do {
                            val event = awaitPointerEvent()
                            event.changes.forEach { it.consume() }
                        } while (event.changes.any { it.pressed })
                        interactions.tryEmit(PressInteraction.Release(press))
                    }
                }
            }
        }
}

private enum class PressEnd { Lifted, Moved, Held }

/** Waits for the finger to lift, or for this touch to stop being a tap. */
private suspend fun AwaitPointerEventScope.pressEnd(id: PointerId, start: Offset, slop: Float): PressEnd {
    while (true) {
        val event = awaitPointerEvent()
        val change = event.changes.firstOrNull { it.id == id } ?: return PressEnd.Moved
        if (change.isConsumed) return PressEnd.Moved
        if (change.changedToUp()) {
            change.consume()
            return PressEnd.Lifted
        }
        if (movedPastSlop(change.position - start, slop)) return PressEnd.Moved
        // A list around the row consumes after this row has looked, so it is seen on the last pass.
        if (awaitPointerEvent(PointerEventPass.Final).changes.any { it.id == id && it.isConsumed }) return PressEnd.Moved
    }
}

internal fun movedPastSlop(moved: Offset, slop: Float): Boolean =
    moved.x * moved.x + moved.y * moved.y > slop * slop
