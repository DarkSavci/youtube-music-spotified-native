package dev.darksavci.spotified.ui.player

import android.graphics.Color as AndroidColor
import android.content.Context
import android.content.Intent
import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Paint
import android.graphics.Typeface
import android.text.Layout
import android.text.StaticLayout
import android.text.TextPaint
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.toArgb
import androidx.core.content.FileProvider
import androidx.core.content.res.ResourcesCompat
import dev.darksavci.spotified.R
import dev.darksavci.spotified.api.Track
import dev.darksavci.spotified.ui.components.darkened
import java.io.File

/** A few lines of a song, drawn as a card on the art's colour, to share as a picture. */
object LyricCard {
    fun share(context: Context, track: Track, lines: List<String>, tint: Color) {
        val width = 1080
        val pad = 96
        val bold = ResourcesCompat.getFont(context, R.font.inter_bold) ?: Typeface.DEFAULT_BOLD
        val regular = ResourcesCompat.getFont(context, R.font.inter_regular) ?: Typeface.DEFAULT
        val words = TextPaint(Paint.ANTI_ALIAS_FLAG).apply { color = AndroidColor.WHITE; textSize = 64f; typeface = bold }
        val small = TextPaint(Paint.ANTI_ALIAS_FLAG).apply { color = AndroidColor.argb(200, 255, 255, 255); textSize = 40f; typeface = regular }
        val text = StaticLayout.Builder.obtain(lines.joinToString("\n"), 0, lines.joinToString("\n").length, words, width - pad * 2)
            .setAlignment(Layout.Alignment.ALIGN_NORMAL).setLineSpacing(12f, 1f).build()
        val credit = "${track.title} · ${track.artistNames}"
        val height = pad * 2 + text.height + 140
        val bitmap = Bitmap.createBitmap(width, height, Bitmap.Config.ARGB_8888)
        val canvas = Canvas(bitmap)
        canvas.drawColor(tint.darkened(0.2f).toArgb())
        canvas.save()
        canvas.translate(pad.toFloat(), pad.toFloat())
        text.draw(canvas)
        canvas.restore()
        canvas.drawText(credit.take(48), pad.toFloat(), (pad + text.height + 100).toFloat(), small)

        val dir = File(context.cacheDir, "share").apply { mkdirs() }
        val file = File(dir, "lyrics.png")
        file.outputStream().use { bitmap.compress(Bitmap.CompressFormat.PNG, 100, it) }
        val uri = FileProvider.getUriForFile(context, "${context.packageName}.files", file)
        val send = Intent(Intent.ACTION_SEND).setType("image/png")
            .putExtra(Intent.EXTRA_STREAM, uri)
            .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        context.startActivity(Intent.createChooser(send, "Share lyrics").addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
    }
}
