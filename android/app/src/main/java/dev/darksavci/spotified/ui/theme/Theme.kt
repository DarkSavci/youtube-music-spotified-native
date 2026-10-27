package dev.darksavci.spotified.ui.theme

import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Typography
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.sp
import dev.darksavci.spotified.R
import dev.darksavci.spotified.data.Theme

/**
 * The desktop's palette (crates/app/src/theme.rs), value for value. Neutral
 * greys, so the artwork carries the colour, and one accent.
 */
@Immutable
data class Palette(
    val dark: Boolean,
    val window: Color,
    val panel: Color,
    val surface: Color,
    val surfaceHover: Color,
    val surfaceActive: Color,
    val outline: Color,
    val text: Color,
    val secondary: Color,
    val dim: Color,
    val accent: Color,
    val onAccent: Color,
    val danger: Color,
    val overlay: Color,
)

val DarkPalette = Palette(
    dark = true,
    window = Color(0xFF0F0F0F),
    panel = Color(0xFF181818),
    surface = Color(0xFF212121),
    surfaceHover = Color(0xFF282828),
    surfaceActive = Color(0xFF383838),
    outline = Color(0xFF2F2F2F),
    text = Color.White,
    secondary = Color(0xFFAAAAAA),
    dim = Color(0xFF717171),
    accent = Color(0xFFFF0033),
    onAccent = Color.White,
    danger = Color(0xFFFF4E45),
    overlay = Color(0xFF282828),
)

/** The dark palette with true black behind it, for OLED screens. */
val BlackPalette = DarkPalette.copy(window = Color.Black, panel = Color.Black, surface = Color(0xFF141414))

val LightPalette = Palette(
    dark = false,
    window = Color(0xFFE4E7EC),
    panel = Color.White,
    surface = Color(0xFFEEF0F3),
    surfaceHover = Color(0xFFE3E6EB),
    surfaceActive = Color(0xFFD7DBE1),
    outline = Color(0xFFDDE1E6),
    text = Color(0xFF14171A),
    secondary = Color(0xFF535B66),
    dim = Color(0xFF8B939E),
    accent = Color(0xFFE6002E),
    onAccent = Color.White,
    danger = Color(0xFFD63B4C),
    overlay = Color.White,
)

val LocalPalette = staticCompositionLocalOf { DarkPalette }

val Inter = FontFamily(
    Font(R.font.inter_regular, FontWeight.Normal),
    Font(R.font.inter_medium, FontWeight.Medium),
    Font(R.font.inter_semibold, FontWeight.SemiBold),
    Font(R.font.inter_bold, FontWeight.Bold),
)

/** The type scale, Spotify's on a phone: bold titles, regular everything else. */
object Type {
    val display = TextStyle(fontFamily = Inter, fontWeight = FontWeight.Bold, fontSize = 32.sp, lineHeight = 38.sp, letterSpacing = (-0.5).sp)
    val title = TextStyle(fontFamily = Inter, fontWeight = FontWeight.Bold, fontSize = 24.sp, lineHeight = 30.sp)
    val heading = TextStyle(fontFamily = Inter, fontWeight = FontWeight.Bold, fontSize = 20.sp, lineHeight = 26.sp)
    val body = TextStyle(fontFamily = Inter, fontWeight = FontWeight.Normal, fontSize = 15.sp, lineHeight = 20.sp)
    val bodyStrong = TextStyle(fontFamily = Inter, fontWeight = FontWeight.SemiBold, fontSize = 15.sp, lineHeight = 20.sp)
    val small = TextStyle(fontFamily = Inter, fontWeight = FontWeight.Normal, fontSize = 13.sp, lineHeight = 17.sp)
    val label = TextStyle(fontFamily = Inter, fontWeight = FontWeight.Medium, fontSize = 13.sp, lineHeight = 16.sp)
    val tiny = TextStyle(fontFamily = Inter, fontWeight = FontWeight.Medium, fontSize = 11.sp, lineHeight = 14.sp)
}

fun paletteOf(theme: Theme) = when (theme) {
    Theme.Dark -> DarkPalette
    Theme.Black -> BlackPalette
    Theme.Light -> LightPalette
}

@Composable
fun SpotifiedTheme(theme: Theme, content: @Composable () -> Unit) {
    val p = paletteOf(theme)
    val scheme = if (p.dark) {
        darkColorScheme(
            primary = p.accent, onPrimary = p.onAccent, background = p.window, onBackground = p.text,
            surface = p.panel, onSurface = p.text, surfaceVariant = p.surface, onSurfaceVariant = p.secondary,
            outline = p.outline, error = p.danger, surfaceContainer = p.panel, surfaceContainerHigh = p.surface,
            surfaceContainerHighest = p.surfaceHover, surfaceContainerLow = p.panel,
        )
    } else {
        lightColorScheme(
            primary = p.accent, onPrimary = p.onAccent, background = p.window, onBackground = p.text,
            surface = p.panel, onSurface = p.text, surfaceVariant = p.surface, onSurfaceVariant = p.secondary,
            outline = p.outline, error = p.danger, surfaceContainer = p.panel, surfaceContainerHigh = p.surface,
            surfaceContainerHighest = p.surfaceHover, surfaceContainerLow = p.panel,
        )
    }
    val typography = Typography(
        bodyLarge = Type.body, bodyMedium = Type.body, bodySmall = Type.small,
        titleLarge = Type.title, titleMedium = Type.heading, titleSmall = Type.bodyStrong,
        labelLarge = Type.label, labelMedium = Type.label, labelSmall = Type.tiny,
        headlineMedium = Type.display, headlineSmall = Type.title,
    )
    // Text given no colour of its own reads in the palette's, not Compose's black.
    CompositionLocalProvider(LocalPalette provides p, LocalContentColor provides p.text) {
        MaterialTheme(colorScheme = scheme, typography = typography, content = content)
    }
}
