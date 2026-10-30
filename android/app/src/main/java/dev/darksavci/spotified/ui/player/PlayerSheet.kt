package dev.darksavci.spotified.ui.player

import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.input.pointer.PointerEventPass
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.awaitEachGesture
import android.os.SystemClock
import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.animate
import androidx.compose.animation.core.spring
import androidx.compose.foundation.gestures.AnchoredDraggableState
import androidx.compose.foundation.gestures.DraggableAnchors
import androidx.compose.foundation.gestures.Orientation
import androidx.compose.foundation.gestures.ScrollScope
import androidx.compose.foundation.gestures.anchoredDraggable
import androidx.compose.foundation.gestures.TargetedFlingBehavior
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.nestedscroll.NestedScrollConnection
import androidx.compose.ui.input.nestedscroll.NestedScrollSource
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.Velocity
import androidx.compose.ui.unit.dp
import kotlin.math.roundToInt
import kotlinx.coroutines.flow.drop
import kotlinx.coroutines.flow.filterNotNull

/** Where a sheet rests. Its offset is 0 open, and its height down the screen closed. */
internal enum class SheetAt { Open, Closed }

/**
 * A full-screen page that follows the finger: the player over the app, and the
 * lyrics over the player. It is moved by [anchoredDraggable] where nothing inside
 * scrolls, and through [connection] by the lists inside it: a list at its top pulls
 * the page down, and a page pulled down rises before the list scrolls again.
 */
@Stable
internal class DragSheet(initial: SheetAt, private val fast: Float, private val flick: Float) {
    val drag = AnchoredDraggableState(initial)

    val offset: Float get() = drag.offset

    /** Moves the closed place, as the screen or the mini player does. */
    fun place(closedAt: Float) {
        drag.updateAnchors(
            DraggableAnchors {
                SheetAt.Open at 0f
                SheetAt.Closed at closedAt
            },
            drag.targetValue,
        )
    }

    private val closedAt: Float
        get() = drag.anchors.let { if (it.hasPositionFor(SheetAt.Closed)) it.positionOf(SheetAt.Closed) else 0f }

    /** How open it is, from 0 closed to 1 open; read it where it is drawn. */
    val openness: Float get() = opennessAt(drag.offset, closedAt)

    /** Where it is resting, or null while it is on its way. */
    val resting: SheetAt?
        get() = when (drag.offset) {
            0f -> SheetAt.Open
            closedAt -> SheetAt.Closed
            else -> null
        }

    /** Goes to [where] from wherever it is, keeping the speed the finger left it with. */
    suspend fun to(where: SheetAt, velocity: Float = 0f) {
        drag.anchoredDrag(where) { anchors, target ->
            val start = drag.offset.takeUnless { it.isNaN() } ?: return@anchoredDrag
            animate(start, anchors.positionOf(target), velocity, MOTION) { value, speed -> dragTo(value, speed) }
        }
    }

    suspend fun settle(velocity: Float) = to(if (settlesOpen(openness, velocity, fast)) SheetAt.Open else SheetAt.Closed, velocity)

    private var downAt = 0f
    private var downTime = 0L

    /** Notes where the sheet was when a finger came down on what drags it. */
    fun fingerDown(time: Long) {
        downAt = drag.offset.takeUnless { it.isNaN() } ?: 0f
        downTime = time
    }

    /**
     * Where a finger let go of the sheet sends it. The speed is read here, not by the
     * library's own rule, so that it can be kept to one thing for both elements that
     * drag the sheet: a flick decides, and otherwise how far it has come from the
     * end it began at.
     */
    val fling = object : TargetedFlingBehavior {
        override suspend fun ScrollScope.performFling(initialVelocity: Float, onRemainingDistanceUpdated: (Float) -> Unit): Float {
            val start = drag.offset.takeUnless { it.isNaN() } ?: return initialVelocity
            val velocity = fingerVelocity(initialVelocity, start - downAt, SystemClock.uptimeMillis() - downTime)
            val closeBelow = if (downAt < closedAt / 2) 1f - SHEET_TRAVEL else SHEET_TRAVEL
            val open = settlesOpen(openness, velocity, flick, closeBelow)
            val target = if (open) 0f else closedAt
            var last = start
            animate(start, target, velocity, MOTION) { value, _ ->
                scrollBy(value - last)
                last = value
            }
            return 0f
        }
    }

    val connection = object : NestedScrollConnection {
        private val pulled get() = drag.offset.let { !it.isNaN() && it > 0.5f }

        override fun onPreScroll(available: Offset, source: NestedScrollSource): Offset =
            if (source == NestedScrollSource.UserInput && available.y < 0f && pulled) Offset(0f, drag.dispatchRawDelta(available.y))
            else Offset.Zero

        // Only a finger pulls: a list flung to its top stops there, and leaves the page be.
        override fun onPostScroll(consumed: Offset, available: Offset, source: NestedScrollSource): Offset =
            if (source == NestedScrollSource.UserInput && available.y > 0f) Offset(0f, drag.dispatchRawDelta(available.y))
            else Offset.Zero

        override suspend fun onPreFling(available: Velocity): Velocity {
            if (!pulled) return Velocity.Zero
            settle(available.y)
            return available
        }

        override suspend fun onPostFling(consumed: Velocity, available: Velocity): Velocity {
            if (!pulled) return Velocity.Zero
            settle(available.y)
            return available
        }
    }

    private companion object {
        val MOTION = spring<Float>(dampingRatio = Spring.DampingRatioNoBouncy, stiffness = Spring.StiffnessMediumLow)
    }
}

@Composable
internal fun rememberDragSheet(initial: SheetAt): DragSheet {
    val density = LocalDensity.current
    return remember { DragSheet(initial, with(density) { 300.dp.toPx() }, with(density) { SHEET_FLICK.toPx() }) }
}

/** What a let-go finger does to a [DragSheet], whichever element it was dragging. */
@Composable
internal fun sheetFling(sheet: DragSheet): TargetedFlingBehavior = sheet.fling

/** What drags [sheet] wears this, for its fling to know where the gesture began. It takes nothing from the gesture. */
internal fun Modifier.dragsSheet(sheet: DragSheet): Modifier = pointerInput(sheet) {
    awaitEachGesture {
        sheet.fingerDown(awaitFirstDown(requireUnconsumed = false, pass = PointerEventPass.Initial).uptimeMillis)
    }
}

/** How fast a let-go finger must be going for its direction to decide, whatever the distance. */
private val SHEET_FLICK = 125.dp

/**
 * A [DragSheet] on the screen. It goes to [open] when that changes, tells [setOpen]
 * when a finger has left it somewhere else, and is only composed while any of it
 * shows. [closedAt] is where its top is when closed, or the full height when null;
 * [fade] fades it in over the first of the way, for one that rises out of the mini player.
 */
@Composable
internal fun SheetLayer(
    sheet: DragSheet,
    open: Boolean,
    setOpen: (Boolean) -> Unit,
    modifier: Modifier = Modifier,
    closedAt: Float? = null,
    fade: Boolean = false,
    content: @Composable () -> Unit,
) {
    val tell by rememberUpdatedState(setOpen)
    BoxWithConstraints(Modifier.fillMaxSize()) {
        val closed = closedAt?.takeIf { it > 0f } ?: constraints.maxHeight.toFloat()
        SideEffect { sheet.place(closed) }
        LaunchedEffect(open, closed > 0f) { sheet.to(if (open) SheetAt.Open else SheetAt.Closed) }
        // The first resting place is only where it began; a finger is what can change it after that.
        LaunchedEffect(sheet) {
            snapshotFlow { sheet.resting }.filterNotNull().drop(1).collect { tell(it == SheetAt.Open) }
        }
        val showing by remember(sheet) { derivedStateOf { sheet.openness > 0f } }
        if (showing) {
            val corner = with(LocalDensity.current) { 16.dp.toPx() }
            Box(
                modifier.fillMaxSize()
                    .offset { IntOffset(0, sheet.offset.takeUnless { it.isNaN() }?.roundToInt() ?: 0) }
                    .graphicsLayer {
                        val on = sheet.openness
                        if (fade) alpha = (on * FADE_OVER).coerceIn(0f, 1f)
                        if (on < 1f) {
                            shape = RoundedCornerShape(corner * (1f - on))
                            clip = true
                        }
                    }
                    .dragsSheet(sheet)
                    .anchoredDraggable(sheet.drag, Orientation.Vertical, flingBehavior = sheetFling(sheet))
                    .nestedScroll(sheet.connection),
            ) { content() }
        }
    }
}

/** A sheet and the mini player cross over the first quarter of the way. */
internal const val FADE_OVER = 4f
