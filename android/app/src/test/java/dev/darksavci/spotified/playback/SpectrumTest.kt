package dev.darksavci.spotified.playback

import androidx.media3.common.C
import androidx.media3.common.audio.AudioProcessor
import java.nio.ByteBuffer
import java.nio.ByteOrder
import kotlin.math.PI
import kotlin.math.sin
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class SpectrumTest {
    private val rate = 44_100
    private val stereo = AudioProcessor.AudioFormat(rate, 2, C.ENCODING_PCM_16BIT)

    private fun sineBytes(hz: Double, frames: Int, amplitude: Double = 0.8): ByteArray {
        val out = ByteBuffer.allocate(frames * 4).order(ByteOrder.nativeOrder())
        for (i in 0 until frames) {
            val s = (sin(2 * PI * hz * i / rate) * amplitude * Short.MAX_VALUE).toInt().toShort()
            out.putShort(s).putShort(s)
        }
        return out.array()
    }

    private fun tap(publish: (FloatArray) -> Unit = {}) = SpectrumTap(publish).apply {
        configure(stereo)
        flush(AudioProcessor.StreamMetadata.DEFAULT)
    }

    private fun SpectrumTap.pass(bytes: ByteArray): ByteArray {
        val input = ByteBuffer.allocateDirect(bytes.size).order(ByteOrder.nativeOrder())
        input.put(bytes).flip()
        queueInput(input)
        assertFalse("the input was left unread", input.hasRemaining())
        val output = getOutput()
        return ByteArray(output.remaining()).also { output.get(it) }
    }

    @Test
    fun theSoundComesOutAsItWentIn() {
        val sound = sineBytes(440.0, 8192)
        assertArrayEquals(sound, tap().pass(sound))
        assertArrayEquals(sound, tap().apply { watching = true }.pass(sound))
    }

    @Test
    fun unwatchedItWorksNothingOut() {
        var published = 0
        val tap = tap { published++ }
        tap.pass(sineBytes(440.0, rate))
        assertEquals(0, published)
    }

    @Test
    fun watchedItPublishesAboutThirtyTimesASecondWhateverTheBuffers() {
        var published = 0
        val tap = tap { published++ }.apply { watching = true }
        val second = sineBytes(440.0, rate)
        val chunk = 4096
        for (from in second.indices step chunk) tap.pass(second.copyOfRange(from, minOf(from + chunk, second.size)))
        assertTrue("published $published", published in 29..31)
    }

    @Test
    fun eachPublishedFrameIsItsOwn() {
        val frames = mutableListOf<FloatArray>()
        val tap = tap { frames += it }.apply { watching = true }
        tap.pass(sineBytes(440.0, rate / 4))
        assertTrue(frames.size > 1)
        assertFalse(frames[0] === frames[1])
    }

    @Test
    fun whatItCannotReadItLetsThrough() {
        val tap = SpectrumTap { }
        val odd = AudioProcessor.AudioFormat(rate, 2, C.ENCODING_PCM_24BIT)
        tap.configure(odd)
        tap.flush(AudioProcessor.StreamMetadata.DEFAULT)
        assertFalse(tap.isActive)
    }

    @Test
    fun aToneLightsTheBandItIsIn() {
        val low = barsOf(100.0)
        val high = barsOf(8000.0)
        assertTrue(low.indices.maxBy { low[it] } < high.indices.maxBy { high[it] })
        assertTrue("the tone is loud", low.max() > 0.8f)
    }

    @Test
    fun aFasterToneLightsAHigherBandThanAMiddleOne() {
        val middle = barsOf(1000.0)
        val high = barsOf(8000.0)
        assertTrue(middle.indices.maxBy { middle[it] } < high.indices.maxBy { high[it] })
    }

    @Test
    fun silenceLightsNothing() {
        val analyser = SpectrumAnalyser().apply { configure(rate) }
        repeat(rate) { analyser.add(0f) }
        assertEquals(0f, analyser.bars.max(), 0f)
    }

    @Test
    fun barsStayBetweenZeroAndOne() {
        val analyser = SpectrumAnalyser().apply { configure(rate) }
        repeat(rate) { analyser.add(if (it % 2 == 0) 1f else -1f) }
        assertTrue(analyser.bars.all { it in 0f..1f })
    }

    private fun barsOf(hz: Double): FloatArray {
        val analyser = SpectrumAnalyser().apply { configure(rate) }
        for (i in 0 until rate / 2) analyser.add((sin(2 * PI * hz * i / rate) * 0.9).toFloat())
        return analyser.bars
    }
}
