package dev.darksavci.spotified.ui.screens

import android.content.Context
import android.content.Intent
import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.LinearGradient
import android.graphics.Paint
import android.graphics.Path
import android.graphics.RectF
import android.graphics.Shader
import android.graphics.Typeface
import android.text.Layout
import android.text.StaticLayout
import android.text.TextPaint
import android.text.TextUtils
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.toArgb
import androidx.core.content.FileProvider
import androidx.core.content.res.ResourcesCompat
import coil3.imageLoader
import coil3.request.ImageRequest
import coil3.request.SuccessResult
import coil3.request.allowHardware
import coil3.toBitmap
import dev.darksavci.spotified.R
import java.io.File
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/** A recap card drawn as a story-sized picture, to share where a story goes. */
internal object RecapImage {
    private const val WIDTH = 1080
    private const val HEIGHT = 1920
    private const val PAD = 96f

    /** Draws the card and opens the share sheet for it. */
    suspend fun share(context: Context, card: RecapCard, label: String, colours: Pair<Color, Color>, photo: String?) {
        val file = withContext(Dispatchers.IO) {
            val bitmap = draw(context, card, label, colours, photo)
            File(File(context.cacheDir, "share").apply { mkdirs() }, "my-year.png").also { f ->
                f.outputStream().use { bitmap.compress(Bitmap.CompressFormat.PNG, 100, it) }
            }
        }
        val uri = FileProvider.getUriForFile(context, "${context.packageName}.files", file)
        val send = Intent(Intent.ACTION_SEND).setType("image/png")
            .putExtra(Intent.EXTRA_STREAM, uri)
            .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        context.startActivity(Intent.createChooser(send, "Share your year").addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
    }

    private suspend fun draw(context: Context, card: RecapCard, label: String, colours: Pair<Color, Color>, photo: String?): Bitmap {
        val bitmap = Bitmap.createBitmap(WIDTH, HEIGHT, Bitmap.Config.ARGB_8888)
        val canvas = Canvas(bitmap)
        val back = Paint(Paint.ANTI_ALIAS_FLAG).apply {
            shader = LinearGradient(0f, 0f, WIDTH.toFloat(), HEIGHT.toFloat(), colours.first.toArgb(), colours.second.toArgb(), Shader.TileMode.CLAMP)
        }
        canvas.drawRect(0f, 0f, WIDTH.toFloat(), HEIGHT.toFloat(), back)
        // Big soft circles, as the card on screen has.
        canvas.drawCircle(WIDTH * 1.05f, HEIGHT * 0.12f, WIDTH * 0.75f, Paint(Paint.ANTI_ALIAS_FLAG).apply { color = android.graphics.Color.argb(20, 255, 255, 255) })
        canvas.drawCircle(-WIDTH * 0.1f, HEIGHT * 0.95f, WIDTH * 0.6f, Paint(Paint.ANTI_ALIAS_FLAG).apply { color = android.graphics.Color.argb(30, 0, 0, 0) })

        val bold = ResourcesCompat.getFont(context, R.font.inter_bold) ?: Typeface.DEFAULT_BOLD
        val regular = ResourcesCompat.getFont(context, R.font.inter_regular) ?: Typeface.DEFAULT
        val white = android.graphics.Color.WHITE
        val soft = android.graphics.Color.argb(215, 255, 255, 255)
        fun paint(size: Float, face: Typeface, colour: Int = white) = TextPaint(Paint.ANTI_ALIAS_FLAG).apply { color = colour; textSize = size; typeface = face }

        canvas.drawText(label, PAD, 170f, paint(40f, bold, soft).apply { letterSpacing = 0.12f })
        canvas.drawText("Youtube Music Spotified", PAD, HEIGHT - 110f, paint(38f, regular, soft))

        when (card) {
            is RecapCard.Time -> {
                var y = HEIGHT * 0.34f
                y = text(canvas, "You listened for", paint(64f, bold, soft), y)
                y = text(canvas, card.value, paint(168f, bold), y + 24f)
                text(canvas, card.caption, paint(64f, bold, soft), y + 32f)
            }
            is RecapCard.Feature -> {
                var y = 340f
                y = text(canvas, card.caption, paint(64f, bold), y, center = true)
                val size = 720f
                val left = (WIDTH - size) / 2
                picture(context, photo ?: card.art)?.let { drawPicture(canvas, it, RectF(left, y + 64f, left + size, y + 64f + size), card.round) }
                y += 64f + size + 64f
                y = text(canvas, card.title, paint(92f, bold), y, lines = 2, center = true)
                text(canvas, card.subtitle, paint(52f, regular, soft), y + 16f, lines = 2, center = true)
            }
            is RecapCard.Top -> {
                var y = text(canvas, card.title, paint(92f, bold), 340f) + 64f
                card.rows.forEachIndexed { i, row ->
                    canvas.drawText("${i + 1}", PAD, y + 118f, paint(92f, bold, android.graphics.Color.argb(150, 255, 255, 255)))
                    val art = 176f
                    val left = PAD + 96f
                    picture(context, row.art)?.let { drawPicture(canvas, it, RectF(left, y, left + art, y + art), round = false) }
                    val at = left + art + 40f
                    val width = WIDTH - PAD - at
                    canvas.drawText(TextUtils.ellipsize(row.title, paint(52f, bold), width, TextUtils.TruncateAt.END).toString(), at, y + 76f, paint(52f, bold))
                    canvas.drawText(TextUtils.ellipsize(row.artist, paint(42f, regular), width, TextUtils.TruncateAt.END).toString(), at, y + 136f, paint(42f, regular, soft))
                    y += art + 56f
                }
            }
        }
        return bitmap
    }

    /** Wrapped text from [y] across the card; where it ends. */
    private fun text(canvas: Canvas, words: String, paint: TextPaint, y: Float, lines: Int = 3, center: Boolean = false): Float {
        val width = (WIDTH - 2 * PAD).toInt()
        val layout = StaticLayout.Builder.obtain(words, 0, words.length, paint, width)
            .setAlignment(if (center) Layout.Alignment.ALIGN_CENTER else Layout.Alignment.ALIGN_NORMAL)
            .setMaxLines(lines).setEllipsize(TextUtils.TruncateAt.END).build()
        canvas.save()
        canvas.translate(PAD, y)
        layout.draw(canvas)
        canvas.restore()
        return y + layout.height
    }

    private suspend fun picture(context: Context, address: String): Bitmap? {
        if (address.isEmpty()) return null
        val request = ImageRequest.Builder(context).data(address).allowHardware(false).build()
        return (context.imageLoader.execute(request) as? SuccessResult)?.image?.toBitmap()
    }

    private fun drawPicture(canvas: Canvas, bitmap: Bitmap, to: RectF, round: Boolean) {
        canvas.save()
        val corner = if (round) to.width() / 2 else 24f
        canvas.clipPath(Path().apply { addRoundRect(to, corner, corner, Path.Direction.CW) })
        // Centre-cropped, so that a wide picture is not squeezed.
        val side = minOf(bitmap.width, bitmap.height)
        val left = (bitmap.width - side) / 2
        val top = (bitmap.height - side) / 2
        canvas.drawBitmap(bitmap, android.graphics.Rect(left, top, left + side, top + side), to, Paint(Paint.ANTI_ALIAS_FLAG or Paint.FILTER_BITMAP_FLAG))
        canvas.restore()
    }
}
