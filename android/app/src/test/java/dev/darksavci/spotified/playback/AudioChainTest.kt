package dev.darksavci.spotified.playback

import androidx.media3.common.C
import androidx.media3.common.PlaybackParameters
import androidx.media3.common.audio.AudioProcessor
import androidx.media3.common.audio.SonicAudioProcessor
import java.nio.ByteBuffer
import java.nio.ByteOrder
import kotlin.random.Random
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/** The processors the sink runs the decoded sound through, as Engine builds them. */
class AudioChainTest {
    private val format = AudioProcessor.AudioFormat(48_000, 2, C.ENCODING_PCM_16BIT)

    /** The chain configured as the sink would for one track; the active processors, in order. */
    private fun active(speed: Float = 1f, skipSilence: Boolean = false): List<AudioProcessor> {
        val chain = audioChain(SpectrumTap {})
        chain.applyPlaybackParameters(PlaybackParameters(speed))
        chain.applySkipSilenceEnabled(skipSilence)
        var input = format
        return chain.audioProcessors.filter { p ->
            val output = p.configure(input)
            p.flush(AudioProcessor.StreamMetadata.DEFAULT)
            if (p.isActive) input = output
            p.isActive
        }
    }

    @Test
    fun atNormalSpeedOnlyTheTapIsInTheWay() {
        val processors = active()
        assertEquals(1, processors.size)
        assertTrue(processors[0] is SpectrumTap)
    }

    @Test
    fun atNormalSpeedTheSamplesComeOutAsTheyWentIn() {
        // Noise over the whole 16-bit range, full scale included.
        val sound = Random(7).nextBytes(48_000 * 4)
        var bytes = sound
        for (p in active()) {
            val input = ByteBuffer.allocateDirect(bytes.size).order(ByteOrder.nativeOrder())
            input.put(bytes).flip()
            p.queueInput(input)
            val output = p.output
            bytes = ByteArray(output.remaining()).also { output.get(it) }
        }
        assertArrayEquals(sound, bytes)
    }

    @Test
    fun speedAndSkippingSilenceAreNoticed() {
        // So the two tests above would see Sonic or the skipper if they were in the way.
        assertTrue(active(speed = 1.5f).any { it is SonicAudioProcessor })
        assertEquals(2, active(skipSilence = true).size)
    }
}
