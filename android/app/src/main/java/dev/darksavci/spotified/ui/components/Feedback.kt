package dev.darksavci.spotified.ui.components

import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.hapticfeedback.HapticFeedback
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalHapticFeedback
import dev.darksavci.spotified.ui.theme.LocalPalette
import dev.darksavci.spotified.ui.theme.Type

/**
 * Asks before something that cannot be taken back. A page shows it while its
 * own state says so; from a menu, which closes first, use `Actions.confirm`.
 */
@Composable
fun ConfirmDialog(
    title: String,
    text: String,
    action: String,
    onConfirm: () -> Unit,
    onDismiss: () -> Unit,
    danger: Boolean = true,
    dismiss: String = "Cancel",
) {
    val p = LocalPalette.current
    AlertDialog(
        onDismissRequest = onDismiss,
        containerColor = p.overlay,
        titleContentColor = p.text,
        textContentColor = p.secondary,
        title = { Text(title, style = Type.heading) },
        text = { Text(text, style = Type.body) },
        confirmButton = {
            TextButton(onConfirm) { Text(action, style = Type.bodyStrong, color = if (danger) p.danger else p.accent) }
        },
        dismissButton = { TextButton(onDismiss) { Text(dismiss, style = Type.bodyStrong, color = p.text) } },
    )
}

/** The small taps of the phone that say a gesture took. */
class Haptics(private val feedback: HapticFeedback) {
    /** `combinedClickable` already does this for its own long press. */
    fun longPress() = feedback.performHapticFeedback(HapticFeedbackType.LongPress)

    /** A drag has been picked up, or a pull has gone far enough. */
    fun dragStart() = feedback.performHapticFeedback(HapticFeedbackType.GestureThresholdActivate)

    /** A dragged row has passed another. */
    fun passed() = feedback.performHapticFeedback(HapticFeedbackType.SegmentTick)

    /** A drag has been let go. */
    fun drop() = feedback.performHapticFeedback(HapticFeedbackType.GestureEnd)

    fun toggle(on: Boolean) = feedback.performHapticFeedback(if (on) HapticFeedbackType.ToggleOn else HapticFeedbackType.ToggleOff)
}

@Composable
fun rememberHaptics(): Haptics {
    val feedback = LocalHapticFeedback.current
    return remember(feedback) { Haptics(feedback) }
}
