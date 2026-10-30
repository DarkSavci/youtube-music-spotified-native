package dev.darksavci.spotified.data

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class SettingsTest {
    @Test
    fun aNewListenerGetsHighOnMobileData() {
        assertEquals("high", Settings().mobileQuality)
        assertEquals("high", mobileQualityOf(null, chosen = false, default = "high"))
    }

    @Test
    fun theOldDefaultWrittenBackByAnotherSettingIsNotAChoice() {
        assertEquals("high", mobileQualityOf("standard", chosen = false, default = "high"))
    }

    @Test
    fun whatWasPickedIsKept() {
        assertEquals("standard", mobileQualityOf("standard", chosen = true, default = "high"))
        assertEquals("low", mobileQualityOf("low", chosen = true, default = "high"))
        // Low was never a default, so it was picked before the choice was noted.
        assertEquals("low", mobileQualityOf("low", chosen = false, default = "high"))
    }

    @Test
    fun theCoreIsToldWhetherTheNetworkCosts() {
        val s = Settings(wifiQuality = "high", mobileQuality = "low")
        assertTrue(s.forCore(metered = true).metered)
        assertEquals("low", s.forCore(metered = true).audioQuality)
        assertFalse(s.forCore(metered = false).metered)
        assertEquals("high", s.forCore(metered = false).audioQuality)
    }
}
