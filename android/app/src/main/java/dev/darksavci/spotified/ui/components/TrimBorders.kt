package dev.darksavci.spotified.ui.components

import android.graphics.Bitmap
import coil3.size.Size
import coil3.transform.Transformation

/**
 * Cuts the plain black bands off a picture's edges. YouTube pads an artist's
 * wide photo into a square with black above and below (and often at the
 * sides), which a round picture shows as black bars.
 */
object TrimBorders : Transformation() {
    override val cacheKey = "trim-borders"

    override suspend fun transform(input: Bitmap, size: Size): Bitmap {
        val w = input.width
        val h = input.height
        val row = IntArray(w)
        val column = IntArray(h)
        fun dark(pixels: IntArray): Boolean = pixels.count { lum(it) > DARK } <= pixels.size / 50
        fun rowDark(y: Int) = input.getPixels(row, 0, w, 0, y, w, 1).let { dark(row) }
        fun columnDark(x: Int) = input.getPixels(column, 0, 1, x, 0, 1, h).let { dark(column) }
        var top = 0
        while (top < h / 2 && rowDark(top)) top++
        var bottom = h - 1
        while (bottom > h / 2 && rowDark(bottom)) bottom--
        var left = 0
        while (left < w / 2 && columnDark(left)) left++
        var right = w - 1
        while (right > w / 2 && columnDark(right)) right--
        val width = right - left + 1
        val height = bottom - top + 1
        // Nothing to cut, or a picture that is mostly black anyway: as it is.
        if (width >= w - 2 && height >= h - 2) return input
        if (width < w / 4 || height < h / 4) return input
        return Bitmap.createBitmap(input, left, top, width, height)
    }

    private fun lum(c: Int): Int = (((c shr 16) and 0xFF) * 3 + ((c shr 8) and 0xFF) * 6 + (c and 0xFF)) / 10

    private const val DARK = 16
}
