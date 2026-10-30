package dev.darksavci.spotified.playback

import androidx.annotation.OptIn
import androidx.media3.common.C
import androidx.media3.common.audio.AudioProcessor
import androidx.media3.common.audio.BaseAudioProcessor
import androidx.media3.common.util.UnstableApi
import java.nio.ByteBuffer
import kotlin.math.PI
import kotlin.math.cos
import kotlin.math.log10
import kotlin.math.max
import kotlin.math.min
import kotlin.math.pow
import kotlin.math.sin
import kotlin.math.sqrt

/**
 * Hands on the audio exactly as it came, and while something is [watching] works
 * out bars from it. It reads the sound the player decoded, so it needs no
 * permission, unlike Android's Visualizer, which is allowed to hear the output.
 * Unwatched, what it costs is the copy every processor makes of its buffer.
 */
@OptIn(UnstableApi::class)
class SpectrumTap(private val publish: (FloatArray) -> Unit) : BaseAudioProcessor() {
    /** Set from the main thread; the audio arrives on the player's. */
    @Volatile
    var watching = false

    private val analyser = SpectrumAnalyser()

    override fun onConfigure(inputAudioFormat: AudioProcessor.AudioFormat): AudioProcessor.AudioFormat {
        if (inputAudioFormat.encoding != C.ENCODING_PCM_16BIT && inputAudioFormat.encoding != C.ENCODING_PCM_FLOAT) {
            return AudioProcessor.AudioFormat.NOT_SET
        }
        analyser.configure(inputAudioFormat.sampleRate)
        return inputAudioFormat
    }

    override fun queueInput(inputBuffer: ByteBuffer) {
        val size = inputBuffer.remaining()
        if (size == 0) return
        if (watching) analyse(inputBuffer)
        replaceOutputBuffer(size).put(inputBuffer).flip()
    }

    /** Reads by index, so the buffer is left as it was found for the copy after. */
    private fun analyse(input: ByteBuffer) {
        val channels = inputAudioFormat.channelCount
        val float = inputAudioFormat.encoding == C.ENCODING_PCM_FLOAT
        val step = if (float) 4 else 2
        var at = input.position()
        val end = input.limit() - step * channels
        while (at <= end) {
            var mono = 0f
            for (c in 0 until channels) {
                mono += if (float) input.getFloat(at) else input.getShort(at) / 32768f
                at += step
            }
            if (analyser.add(mono / channels)) publish(analyser.bars.copyOf())
        }
    }

    override fun onFlush(streamMetadata: AudioProcessor.StreamMetadata) = analyser.clear()

    override fun onReset() = analyser.clear()
}

/**
 * Samples into a few bars, each 0 to 1: a windowed Fourier transform of the latest
 * samples, gathered into bands spaced as hearing spaces them, about thirty times a
 * second. Its arrays are made once, so nothing is allocated as samples arrive. For
 * one thread at a time.
 */
class SpectrumAnalyser(private val bandCount: Int = BANDS) {
    private val ring = FloatArray(SIZE)
    private var ringAt = 0
    private var sinceFrame = 0
    private var hop = SIZE
    private val re = FloatArray(SIZE)
    private val im = FloatArray(SIZE)
    private val hann = FloatArray(SIZE) { (0.5 - 0.5 * cos(2 * PI * it / (SIZE - 1))).toFloat() }
    private val cosines = FloatArray(SIZE / 2) { cos(2 * PI * it / SIZE).toFloat() }
    private val sines = FloatArray(SIZE / 2) { sin(2 * PI * it / SIZE).toFloat() }
    private val reversed = IntArray(SIZE) { Integer.reverse(it) ushr (32 - Integer.numberOfTrailingZeros(SIZE)) }
    private val edges = IntArray(bandCount + 1)

    /** The bars as last worked out. Overwritten at the next frame: copy to keep them. */
    val bars = FloatArray(bandCount)

    fun configure(sampleRate: Int) {
        hop = (sampleRate / FRAMES_PER_SECOND).coerceAtLeast(1)
        val top = min(HIGHEST_HZ, sampleRate / 2f)
        val ratio = top / LOWEST_HZ
        val binHz = sampleRate.toFloat() / SIZE
        for (i in 0..bandCount) {
            val hz = LOWEST_HZ * ratio.pow(i.toFloat() / bandCount)
            edges[i] = (hz / binHz).toInt().coerceIn(1, SIZE / 2)
        }
        clear()
    }

    fun clear() {
        ring.fill(0f)
        bars.fill(0f)
        ringAt = 0
        sinceFrame = 0
    }

    /** Takes a sample; true when [bars] have just been worked out afresh. */
    fun add(sample: Float): Boolean {
        ring[ringAt] = sample
        ringAt = (ringAt + 1) and (SIZE - 1)
        if (++sinceFrame < hop) return false
        sinceFrame = 0
        frame()
        return true
    }

    private fun frame() {
        for (i in 0 until SIZE) {
            re[reversed[i]] = ring[(ringAt + i) and (SIZE - 1)] * hann[i]
            im[i] = 0f
        }
        transform()
        for (band in 0 until bandCount) {
            val from = edges[band]
            // The low bands are narrower than a bin: each still has one to show.
            val until = max(edges[band + 1], from + 1).coerceAtMost(SIZE / 2)
            var peak = 0f
            for (bin in from until until) peak = max(peak, re[bin] * re[bin] + im[bin] * im[bin])
            // Full scale through a Hann window comes to a quarter of the window's length.
            val level = sqrt(peak) / (SIZE / 4f)
            val db = if (level <= 0f) FLOOR_DB else 20 * log10(level)
            val shown = ((db - FLOOR_DB) / -FLOOR_DB).coerceIn(0f, 1f)
            // Up at once, down gently, as a meter falls.
            bars[band] = if (shown >= bars[band]) shown else bars[band] * FALL + shown * (1 - FALL)
        }
    }

    /** A radix-2 Fourier transform of [re] and [im], in place. */
    private fun transform() {
        var length = 2
        while (length <= SIZE) {
            val half = length / 2
            val stride = SIZE / length
            var start = 0
            while (start < SIZE) {
                for (k in 0 until half) {
                    val w = k * stride
                    val a = start + k
                    val b = a + half
                    val tr = re[b] * cosines[w] + im[b] * sines[w]
                    val ti = im[b] * cosines[w] - re[b] * sines[w]
                    re[b] = re[a] - tr
                    im[b] = im[a] - ti
                    re[a] += tr
                    im[a] += ti
                }
                start += length
            }
            length *= 2
        }
    }

    companion object {
        /** How many bars there are. */
        const val BANDS = 32
        const val LOWEST_HZ = 40f
        const val HIGHEST_HZ = 16_000f
        private const val SIZE = 1024
        private const val FRAMES_PER_SECOND = 30
        private const val FLOOR_DB = -60f
        private const val FALL = 0.75f
    }
}
