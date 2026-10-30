package dev.darksavci.spotified.widget

import android.content.Context
import android.graphics.Bitmap
import androidx.compose.runtime.Composable
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.glance.ColorFilter
import androidx.glance.GlanceModifier
import androidx.glance.Image
import androidx.glance.ImageProvider
import androidx.glance.action.Action
import androidx.glance.action.clickable
import androidx.glance.layout.padding
import androidx.glance.layout.size
import androidx.glance.unit.ColorProvider
import coil3.imageLoader
import coil3.request.ImageRequest
import coil3.request.SuccessResult
import coil3.request.allowHardware
import coil3.toBitmap
import dev.darksavci.spotified.api.Artwork
import dev.darksavci.spotified.api.url
import dev.darksavci.spotified.data.Theme
import dev.darksavci.spotified.ui.theme.paletteOf

/**
 * The widgets' colours: the app's own theme, as chosen in Settings. The app is
 * neutral greys with one accent and keeps to them everywhere, so a widget in the
 * wallpaper's colours would look like another app's.
 */
internal class WidgetColors(theme: Theme) {
    private val palette = paletteOf(theme)
    val background = palette.panel
    val text = ColorProvider(palette.text)
    val secondary = ColorProvider(palette.secondary)
    val accent = ColorProvider(palette.accent)
}

/** An icon that is a button, big enough to hit on a home screen, and named for TalkBack. */
@Composable
internal fun WidgetButton(icon: Int, description: String, tint: ColorProvider, action: Action, size: Dp = 44.dp) {
    Image(
        ImageProvider(icon), description,
        GlanceModifier.size(size).padding(10.dp).clickable(action),
        colorFilter = ColorFilter.tint(tint),
    )
}

/**
 * A cover for a widget, or null. A widget's pictures travel to the launcher in
 * one message with a size limit, so a list's are small and, with [small], take half
 * the memory each.
 */
internal suspend fun widgetBitmap(context: Context, artwork: List<Artwork>, px: Int, small: Boolean = false): Bitmap? {
    val address = artwork.url(px) ?: return null
    val request = ImageRequest.Builder(context).data(address).allowHardware(false).size(px).build()
    val bitmap = runCatching { (context.imageLoader.execute(request) as? SuccessResult)?.image?.toBitmap() }.getOrNull() ?: return null
    return if (small) bitmap.copy(Bitmap.Config.RGB_565, false) ?: bitmap else bitmap
}
