package dev.darksavci.spotified.playback

import android.media.audiofx.AudioEffect
import android.media.audiofx.BassBoost
import android.media.audiofx.Equalizer
import android.media.audiofx.LoudnessEnhancer
import dev.darksavci.spotified.data.Logs
import dev.darksavci.spotified.data.Settings

/**
 * The phone's sound effects on the engine's one audio session, so they act on
 * whichever deck plays: the equalizer, the volume boost and the bass. A phone
 * may lack any of them, or take one away for another app's use, and music
 * plays on regardless, so nothing here is allowed to fail outward.
 */
class SoundEffects(private val session: Int) {
    private var equalizer: Equalizer? = null
    private var enhancer: LoudnessEnhancer? = null
    private var bass: BassBoost? = null

    fun apply(s: Settings) {
        equalizer = switch("equalizer", s.equalizerOn, equalizer, { Equalizer(0, session) }) { applyBands(it, s.equalizerBands) }
        enhancer = switch("boost", s.volumeBoost, enhancer, { LoudnessEnhancer(session) }) { it.setTargetGain(BOOST_MB) }
        bass = switch("bass", s.bassBoost, bass, { BassBoost(0, session) }) { it.setStrength(BASS_STRENGTH) }
    }

    /** Made when first wanted, and kept: it is cheap while off, and switched back on at once. */
    private fun <E : AudioEffect> switch(name: String, on: Boolean, effect: E?, make: () -> E, tune: (E) -> Unit): E? {
        if (!on) {
            runCatching { effect?.enabled = false }
            return effect
        }
        val ready = effect ?: runCatching(make).onFailure { Logs.w(TAG, "no $name on this phone: ${it.message}") }.getOrNull()
        ready?.let { e ->
            runCatching {
                tune(e)
                e.enabled = true
            }.onFailure { Logs.w(TAG, "$name could not be set: ${it.message}") }
        }
        return ready
    }

    private fun applyBands(eq: Equalizer, bands: String) {
        val levels = bands.split(',').mapNotNull { it.trim().toIntOrNull() }
        val (lo, hi) = eq.bandLevelRange.let { it[0].toInt() to it[1].toInt() }
        for (band in 0 until eq.numberOfBands) {
            eq.setBandLevel(band.toShort(), (levels.getOrNull(band) ?: 0).coerceIn(lo, hi).toShort())
        }
    }

    fun release() {
        listOfNotNull(equalizer, enhancer, bass).forEach { runCatching { it.release() } }
        equalizer = null
        enhancer = null
        bass = null
    }

    private companion object {
        const val TAG = "SoundEffects"

        /** 6 dB, in millibels: what the desktop's 200% is. */
        const val BOOST_MB = 600

        /** Out of 1000: clearly heard, short of the boom of the full strength. */
        const val BASS_STRENGTH: Short = 600
    }
}
