package dev.darksavci.spotified.ui.screens

import android.content.Context
import android.net.Uri
import androidx.annotation.DrawableRes
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.toggleableState
import androidx.compose.ui.state.ToggleableState
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import dev.darksavci.spotified.R
import dev.darksavci.spotified.ui.components.Glyph
import dev.darksavci.spotified.ui.components.ThinSlider
import dev.darksavci.spotified.ui.components.tapRow
import dev.darksavci.spotified.ui.theme.LocalPalette
import dev.darksavci.spotified.ui.theme.Type

/** The one place a row of these pages is pressed: a tap, which a sideways swipe over it is not. */
internal fun Modifier.pressRow(role: Role = Role.Button, label: String? = null, enabled: Boolean = true, onClick: () -> Unit): Modifier =
    tapRow(onClick, enabled = enabled, role = role, onClickLabel = label)

@Composable
internal fun Line(title: String, subtitle: String) {
    Column(Modifier.fillMaxWidth().semantics(mergeDescendants = true) {}.padding(horizontal = 16.dp, vertical = 12.dp)) {
        Text(title, style = Type.body.copy(fontSize = 16.sp))
        if (subtitle.isNotEmpty()) Text(subtitle, style = Type.small, color = LocalPalette.current.secondary)
    }
}

/** A row that does something: an icon, what it does, and a chevron when it opens a page. */
@Composable
internal fun Action(title: String, subtitle: String? = null, @DrawableRes icon: Int? = null, chevron: Boolean = true, badge: Boolean = false, onClick: () -> Unit) {
    val p = LocalPalette.current
    Row(
        Modifier.fillMaxWidth().pressRow(onClick = onClick).padding(horizontal = 16.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (icon != null) {
            Glyph(icon, null, tint = p.text, size = 22.dp)
            Spacer(Modifier.width(16.dp))
        }
        Column(Modifier.weight(1f)) {
            Text(title, style = Type.body.copy(fontSize = 16.sp))
            if (!subtitle.isNullOrEmpty()) Text(subtitle, style = Type.small, color = p.secondary)
        }
        if (badge) {
            Box(Modifier.size(8.dp).clip(RoundedCornerShape(4.dp)).background(p.accent))
            Spacer(Modifier.width(8.dp))
        }
        if (chevron) Glyph(R.drawable.ic_chevron_right, null, tint = p.secondary, size = 20.dp)
    }
}

@Composable
internal fun switchColors() = LocalPalette.current.let { p ->
    SwitchDefaults.colors(
        checkedTrackColor = p.accent, checkedThumbColor = Color.White, checkedBorderColor = Color.Transparent,
        uncheckedTrackColor = p.surfaceActive, uncheckedThumbColor = p.secondary, uncheckedBorderColor = Color.Transparent,
    )
}

@Composable
internal fun Toggle(title: String, subtitle: String, on: Boolean, onChange: (Boolean) -> Unit) {
    val p = LocalPalette.current
    Row(
        Modifier.fillMaxWidth().tapRow({ onChange(!on) }, role = Role.Switch)
            .semantics { toggleableState = ToggleableState(on) }.padding(horizontal = 16.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f).padding(end = 12.dp)) {
            Text(title, style = Type.body.copy(fontSize = 16.sp))
            Text(subtitle, style = Type.small, color = p.secondary)
        }
        Switch(on, null, colors = switchColors())
    }
}

/** One of a few options, as a segmented control under its title. */
@Composable
internal fun <T> Choice(title: String, options: List<Pair<T, String>>, current: T, subtitle: String? = null, onPick: (T) -> Unit) {
    val p = LocalPalette.current
    Column(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp)) {
        Text(title, style = Type.body.copy(fontSize = 16.sp), modifier = Modifier.semantics { heading() })
        if (!subtitle.isNullOrEmpty()) Text(subtitle, style = Type.small, color = p.secondary)
        Spacer(Modifier.size(8.dp))
        Row(Modifier.fillMaxWidth().clip(RoundedCornerShape(10.dp)).background(p.surface).padding(4.dp).selectableGroup()) {
            options.forEach { (value, label) ->
                val on = value == current
                Box(
                    Modifier.weight(1f).clip(RoundedCornerShape(8.dp)).background(if (on) p.surfaceActive else Color.Transparent)
                        .tapRow({ onPick(value) }, role = Role.RadioButton).semantics { selected = on }.padding(vertical = 10.dp),
                    contentAlignment = Alignment.Center,
                ) {
                    Text(label, style = if (on) Type.label.copy(fontWeight = FontWeight.Bold) else Type.label, color = if (on) p.text else p.secondary, maxLines = 1)
                }
            }
        }
    }
}

@Composable
internal fun SliderRow(
    title: String, value: String, current: Float, range: ClosedFloatingPointRange<Float>,
    subtitle: String? = null, onDone: (Float) -> Unit,
) {
    val p = LocalPalette.current
    var v by remember(current) { mutableFloatStateOf(current) }
    Column(Modifier.padding(horizontal = 16.dp, vertical = 8.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Column(Modifier.weight(1f)) {
                Text(title, style = Type.body.copy(fontSize = 16.sp))
                if (!subtitle.isNullOrEmpty()) Text(subtitle, style = Type.small, color = p.secondary)
            }
            Text(value, style = Type.bodyStrong, color = p.text)
        }
        // Whole steps: seconds of crossfade, half-gigabytes of cache.
        ThinSlider(v, { v = Math.round(it).toFloat() }, { onDone(v) }, range, description = title)
    }
}

/** A wait that must not be walked away from, such as the core starting on another account. */
@Composable
internal fun BusyDialog(message: String) {
    val p = LocalPalette.current
    Dialog({}, DialogProperties(dismissOnBackPress = false, dismissOnClickOutside = false)) {
        Row(
            Modifier.clip(RoundedCornerShape(16.dp)).background(p.overlay).padding(24.dp)
                .semantics(mergeDescendants = true) { liveRegion = LiveRegionMode.Polite },
            verticalAlignment = Alignment.CenterVertically,
        ) {
            CircularProgressIndicator(color = p.accent, strokeWidth = 3.dp, modifier = Modifier.size(28.dp))
            Spacer(Modifier.width(16.dp))
            Text(message, style = Type.body, color = p.text)
        }
    }
}

/** The file at [uri], or null past [most] bytes: a wrong file is not read into memory. */
internal fun readLimited(context: Context, uri: Uri, most: Int): ByteArray? =
    context.contentResolver.openInputStream(uri)?.use { input ->
        val out = java.io.ByteArrayOutputStream()
        val buffer = ByteArray(16 * 1024)
        while (true) {
            val n = input.read(buffer)
            if (n < 0) break
            out.write(buffer, 0, n)
            if (out.size() > most) return null
        }
        out.toByteArray()
    }
