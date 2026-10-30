package dev.darksavci.spotified.ui.player

import android.content.Context
import android.content.Intent
import android.media.AudioDeviceCallback
import android.media.AudioDeviceInfo
import android.media.AudioManager
import android.media.MediaRouter2
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.provider.Settings
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import dev.darksavci.spotified.R
import dev.darksavci.spotified.ui.components.Glyph
import dev.darksavci.spotified.ui.components.tapRow
import dev.darksavci.spotified.ui.theme.Type

/**
 * Opens the system's picker of where the sound goes: phone, wired, Bluetooth and
 * later Cast, without going through the notification shade. Android 14 has a call
 * for it; before that it is a request to System UI, which is what the androidx
 * media router itself sends. The sound settings are the last resort.
 */
internal fun openOutputPicker(context: Context) {
    if (Build.VERSION.SDK_INT >= 34 && MediaRouter2.getInstance(context).showSystemOutputSwitcher()) return
    val pkg = context.packageName
    val shown = runCatching {
        when {
            Build.VERSION.SDK_INT >= 31 -> context.sendBroadcast(
                Intent("com.android.systemui.action.LAUNCH_MEDIA_OUTPUT_DIALOG")
                    .setPackage("com.android.systemui").putExtra("package_name", pkg),
            )
            Build.VERSION.SDK_INT >= 30 -> context.startActivity(
                Intent("com.android.settings.panel.action.MEDIA_OUTPUT")
                    .putExtra("com.android.settings.panel.extra.PACKAGE_NAME", pkg)
                    .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK),
            )
            else -> error("no picker")
        }
    }.isSuccess
    if (!shown) {
        runCatching { context.startActivity(Intent(Settings.ACTION_SOUND_SETTINGS).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)) }
    }
}

private val EXTERNAL = setOf(
    AudioDeviceInfo.TYPE_BLUETOOTH_A2DP,
    AudioDeviceInfo.TYPE_BLE_HEADSET,
    AudioDeviceInfo.TYPE_BLE_SPEAKER,
    AudioDeviceInfo.TYPE_WIRED_HEADPHONES,
    AudioDeviceInfo.TYPE_WIRED_HEADSET,
    AudioDeviceInfo.TYPE_USB_HEADSET,
    AudioDeviceInfo.TYPE_USB_DEVICE,
    AudioDeviceInfo.TYPE_HDMI,
)

/**
 * The name of the headphones or speaker connected, or null for the phone itself.
 * Android sends music to what was connected last, so this says what is connected
 * rather than asking what is routed; the picker has the last word.
 */
@Composable
private fun rememberOutputName(): String? {
    val context = LocalContext.current
    var name by remember { mutableStateOf<String?>(null) }
    DisposableEffect(context) {
        val audio = context.getSystemService(AudioManager::class.java)
        fun read() {
            name = audio.getDevices(AudioManager.GET_DEVICES_OUTPUTS)
                .lastOrNull { it.type in EXTERNAL }
                ?.let { it.productName?.toString()?.ifBlank { null } ?: "Headphones" }
        }
        val watch = object : AudioDeviceCallback() {
            override fun onAudioDevicesAdded(added: Array<out AudioDeviceInfo>?) = read()
            override fun onAudioDevicesRemoved(removed: Array<out AudioDeviceInfo>?) = read()
        }
        read()
        audio.registerAudioDeviceCallback(watch, Handler(Looper.getMainLooper()))
        onDispose { audio.unregisterAudioDeviceCallback(watch) }
    }
    return name
}

/** The button that says where the sound goes and changes it. With [named], it says the name beside its icon. */
@Composable
internal fun OutputButton(tint: Color = Color.White, named: Boolean = false) {
    val context = LocalContext.current
    val name = rememberOutputName()
    val external = name != null
    val description = if (external) "Playing on $name. Choose where it plays" else "Choose where it plays"
    Row(
        Modifier.heightIn(min = 48.dp).widthIn(min = 48.dp).clip(RoundedCornerShape(24.dp))
            .tapRow(onClick = { openOutputPicker(context) }, role = Role.Button, onClickLabel = description)
            .padding(horizontal = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Glyph(if (external) R.drawable.ic_headphones else R.drawable.ic_speaker, description, tint = tint)
        if (named && name != null) {
            Spacer(Modifier.width(6.dp))
            Text(name, style = Type.small, color = tint, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.widthIn(max = 84.dp))
        }
    }
}
