package dev.darksavci.spotified.playback

import dev.darksavci.spotified.data.Settings
import org.junit.Assert.assertEquals
import org.junit.Test

class SoundEffectsTest {
    private val none = WantedEffects(equalizer = false, boost = false, bass = false)

    @Test
    fun outOfTheBoxNoEffectIsOn() {
        assertEquals(none, wantedEffects(Settings()))
    }

    @Test
    fun aFlatEqualizerIsLeftOff() {
        assertEquals(none, wantedEffects(Settings(equalizerOn = true)))
        assertEquals(none, wantedEffects(Settings(equalizerOn = true, equalizerBands = "0,0,0,0,0")))
        assertEquals(none, wantedEffects(Settings(equalizerOn = true, equalizerBands = " 0, 0,x,")))
    }

    @Test
    fun aShapedEqualizerIsOnOnlyWhenSwitchedOn() {
        assertEquals(none.copy(equalizer = true), wantedEffects(Settings(equalizerOn = true, equalizerBands = "0,300,0,0,-150")))
        assertEquals(none, wantedEffects(Settings(equalizerOn = false, equalizerBands = "0,300,0,0,-150")))
    }

    @Test
    fun boostAndBassFollowTheirSwitches() {
        assertEquals(none.copy(boost = true), wantedEffects(Settings(volumeBoost = true)))
        assertEquals(none.copy(bass = true), wantedEffects(Settings(bassBoost = true)))
    }

    @Test
    fun bandsAreReadAsMillibels() {
        assertEquals(listOf(0, 300, -150), bandLevels("0, 300 ,-150"))
        assertEquals(emptyList<Int>(), bandLevels(""))
    }
}
